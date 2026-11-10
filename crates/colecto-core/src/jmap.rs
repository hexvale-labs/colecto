//! Prototype JMAP transport. Tokens remain in the trusted core.
use crate::{Error, mime::Attachment};
use colecto_extension_api::{DraftInput, Message};
use reqwest::{Url, blocking::Client};
use serde_json::{Value, json};
use std::{io::Read, time::Duration};

const CORE: &str = "urn:ietf:params:jmap:core";
const MAIL: &str = "urn:ietf:params:jmap:mail";
const SUBMISSION: &str = "urn:ietf:params:jmap:submission";

pub struct JmapAccount {
    pub session_url: String,
    pub token: String,
    pub address: String,
}

fn failed() -> Error {
    // Never expose server responses, tokens, URLs, or message contents.
    Error::Transport("JMAP operation failed. Check settings, credentials, and connection.".into())
}
fn http_error(error: reqwest::Error) -> Error {
    if error.is_timeout() {
        Error::Transport(
            "Mail server timed out. Your cached mail is still available; try refreshing again."
                .into(),
        )
    } else {
        failed()
    }
}
fn secure_url(value: &str) -> Result<Url, Error> {
    let url = Url::parse(value).map_err(|_| failed())?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(Error::Invalid(
            "JMAP requires an HTTPS URL without embedded credentials or fragment".into(),
        ));
    }
    Ok(url)
}
fn field<'a>(value: &'a Value, key: &str) -> Result<&'a str, Error> {
    value[key].as_str().ok_or_else(failed)
}
fn list(value: &Value) -> Result<&Vec<Value>, Error> {
    value["list"].as_array().ok_or_else(failed)
}
fn decode(response: reqwest::blocking::Response) -> Result<Value, Error> {
    if !response.status().is_success() {
        let code = response.status().as_u16();
        let detail = match code {
            401 | 403 => "authentication or mailbox access was rejected",
            429 => "too many requests; wait before refreshing",
            500..=599 => "the provider is temporarily unavailable",
            _ => "the provider rejected the request",
        };
        return Err(Error::Transport(format!("JMAP HTTP {code}: {detail}.")));
    }
    let mut bytes = Vec::new();
    response
        .take(8_000_001)
        .read_to_end(&mut bytes)
        .map_err(|_| failed())?;
    if bytes.len() > 8_000_000 {
        return Err(failed());
    }
    serde_json::from_slice(&bytes).map_err(|_| failed())
}

struct Session<'a> {
    account: &'a JmapAccount,
    client: Client,
    api: Url,
    download: String,
    id: String,
    submission: bool,
}
impl JmapAccount {
    pub fn validate(&self) -> Result<(), Error> {
        secure_url(&self.session_url)?;
        if self.token.trim().is_empty() || self.token.contains(['\r', '\n']) {
            return Err(Error::Invalid("Configure a JMAP bearer token".into()));
        }
        self.address
            .parse::<lettre::Address>()
            .map_err(|_| Error::Invalid("Invalid sender address".into()))?;
        Ok(())
    }
    fn session(&self) -> Result<Session<'_>, Error> {
        self.validate()?;
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|_| failed())?;
        let data = decode(
            client
                .get(secure_url(&self.session_url)?)
                .bearer_auth(&self.token)
                .send()
                .map_err(http_error)?,
        )?;
        let id = field(&data["primaryAccounts"], MAIL)?.to_owned();
        if !data["accounts"][&id]["accountCapabilities"][MAIL].is_object() {
            return Err(failed());
        }
        Ok(Session {
            account: self,
            client,
            api: secure_url(field(&data, "apiUrl")?)?,
            download: data["downloadUrl"].as_str().unwrap_or_default().to_owned(),
            submission: data["accounts"][&id]["accountCapabilities"][SUBMISSION].is_object(),
            id,
        })
    }
    pub fn query(&self, folder: &str, search: &str) -> Result<Vec<Message>, Error> {
        crate::validate_query(folder, search)?;
        self.session()?.query(folder, search)
    }
    /// Download one complete message as RFC 5322 bytes.
    pub(crate) fn fetch_raw(&self, id: &str) -> Result<Vec<u8>, Error> {
        let id = id.strip_prefix("jmap-").ok_or(Error::Missing)?;
        let session = self.session()?;
        let data = session.call(
            "Email/get",
            json!({"ids":[id],"properties":["blobId","size"]}),
        )?;
        let email = list(&data)?.first().ok_or(Error::Missing)?;
        if email["size"].as_u64().unwrap_or(u64::MAX) > crate::mime::MAX_RAW as u64 {
            return Err(Error::Invalid(
                "This message is larger than 30 MB and cannot be opened here".into(),
            ));
        }
        // JMAP Ids are URL-safe; refuse anything else rather than escaping it into a URL.
        let blob = field(email, "blobId")?;
        if [blob, session.id.as_str()].iter().any(|value| {
            value.is_empty()
                || !value
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        }) {
            return Err(failed());
        }
        let url = session
            .download
            .replace("{accountId}", &session.id)
            .replace("{blobId}", blob)
            .replace("{type}", "message%2Frfc822")
            .replace("{name}", "message.eml");
        let response = session
            .client
            .get(secure_url(&url)?)
            .bearer_auth(&self.token)
            .send()
            .map_err(http_error)?;
        if !response.status().is_success() {
            return Err(failed());
        }
        let mut bytes = Vec::new();
        response
            .take(crate::mime::MAX_RAW as u64 + 1)
            .read_to_end(&mut bytes)
            .map_err(|_| failed())?;
        if bytes.len() > crate::mime::MAX_RAW {
            return Err(failed());
        }
        Ok(bytes)
    }
    pub(crate) fn send(&self, input: &DraftInput, attachments: &[Attachment]) -> Result<(), Error> {
        if !attachments.is_empty() {
            return Err(Error::Invalid(
                "Attachments are not supported for JMAP accounts yet".into(),
            ));
        }
        self.send_inner(input).map_err(|error| match error {
            Error::Transport(message) => Error::Transport(format!("{message} Sending may have an uncertain outcome; check Sent and delivery status before sending again.")),
            error => error,
        })
    }
    fn send_inner(&self, input: &DraftInput) -> Result<(), Error> {
        self.validate()?;
        crate::validate_draft(input)?;
        let addresses = |list: &str| -> Result<Vec<Value>, Error> {
            Ok(crate::mailboxes(list)?
                .into_iter()
                .map(|mailbox| json!({"name": mailbox.name, "email": mailbox.email.to_string()}))
                .collect())
        };
        let message_ids = |value: &str| -> Value {
            value
                .split_whitespace()
                .map(|id| id.trim_matches(['<', '>']))
                .collect::<Vec<_>>()
                .into()
        };
        let mut email = json!({
            "from":[{"email":self.address}],"to":addresses(&input.recipient)?,"subject":input.subject,
            "textBody":[{"partId":"text","type":"text/plain"}],"bodyValues":{"text":{"value":input.body}}
        });
        if !input.cc.trim().is_empty() {
            email["cc"] = json!(addresses(&input.cc)?);
        }
        if !input.in_reply_to.is_empty() {
            email["inReplyTo"] = message_ids(&input.in_reply_to);
        }
        if !input.references.is_empty() {
            email["references"] = message_ids(&input.references);
        }
        let session = self.session()?;
        if !session.submission {
            return Err(Error::Invalid(
                "JMAP account does not support sending".into(),
            ));
        }
        let identities = session.call("Identity/get", json!({}))?;
        let identity = list(&identities)?
            .iter()
            .find(|v| {
                v["email"]
                    .as_str()
                    .is_some_and(|s| s.eq_ignore_ascii_case(&self.address))
            })
            .ok_or_else(failed)?;
        let drafts = session.mailbox("drafts")?;
        let sent = session.mailbox("sent")?;
        email["mailboxIds"] = json!({(drafts.clone()): true});
        email["keywords"] = json!({"$draft": true});
        let created = session.call("Email/set", json!({"create":{"message":email}}))?;
        let email = field(&created["created"]["message"], "id")?;
        let result = session.call("EmailSubmission/set", json!({"create":{"submission":{
            "identityId":field(identity,"id")?,"emailId":email}},
            "onSuccessUpdateEmail":{"#submission":{
                (format!("mailboxIds/{drafts}")):null,(format!("mailboxIds/{sent}")):true,"keywords/$draft":null
            }}}))?;
        field(&result["created"]["submission"], "id")?;
        Ok(())
    }
}
impl Session<'_> {
    fn query(&self, folder: &str, search: &str) -> Result<Vec<Message>, Error> {
        let mailbox = self.mailbox(folder)?;
        let mut filter = json!({"inMailbox":mailbox});
        if !search.is_empty() {
            filter["text"] = json!(search);
        }
        // shortcut: latest 100 messages only; add changes/pagination before full sync.
        let query = self.call(
            "Email/query",
            json!({"filter":filter,
            "sort":[{"property":"receivedAt","isAscending":false}],"limit":100}),
        )?;
        let ids = query["ids"].as_array().ok_or_else(failed)?;
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let data = self.call(
            "Email/get",
            json!({"ids":ids,
            "properties":["id","from","subject","receivedAt","textBody","bodyValues"],
            "fetchTextBodyValues":true,"maxBodyValueBytes":100000}),
        )?;
        let messages = list(&data)?;
        ids.iter()
            .map(|id| {
                let message = messages
                    .iter()
                    .find(|v| v["id"] == *id)
                    .ok_or_else(failed)?;
                parse_message(message)
            })
            .collect()
    }

    fn call(&self, method: &str, mut args: Value) -> Result<Value, Error> {
        args["accountId"] = json!(self.id);
        let mut using = vec![CORE, MAIL];
        if method.starts_with("Identity/") || method.starts_with("EmailSubmission/") {
            using.push(SUBMISSION);
        }
        let data = decode(
            self.client
                .post(self.api.clone())
                .bearer_auth(&self.account.token)
                .json(&json!({"using":using,"methodCalls":[[method,args,"call"]]}))
                .send()
                .map_err(http_error)?,
        )?;
        method_result(&data, method)
    }
    fn mailbox(&self, role: &str) -> Result<String, Error> {
        let data = self.call("Mailbox/get", json!({"properties":["id","role"]}))?;
        let mailbox = list(&data)?
            .iter()
            .find(|v| v["role"] == role)
            .ok_or_else(failed)?;
        Ok(field(mailbox, "id")?.to_owned())
    }
}
fn method_result(data: &Value, method: &str) -> Result<Value, Error> {
    let responses = data["methodResponses"].as_array().ok_or_else(failed)?;
    if let Some(error) = responses.iter().find(|v| v[0] == "error" && v[2] == "call") {
        let detail = match error[1]["type"].as_str() {
            Some("forbidden") => "JMAP forbidden: mailbox access was rejected",
            Some("serverUnavailable") => "JMAP serverUnavailable: provider temporarily unavailable",
            Some("accountReadOnly") => "JMAP accountReadOnly: account does not allow changes",
            Some("invalidArguments") => {
                "JMAP invalidArguments: provider rejected the request parameters"
            }
            _ => "JMAP provider rejected the request",
        };
        return Err(Error::Transport(format!("{detail}.")));
    }
    let response = responses
        .iter()
        .find(|v| v[0] == method && v[2] == "call")
        .ok_or_else(failed)?;
    Ok(response[1].clone())
}
fn parse_message(value: &Value) -> Result<Message, Error> {
    let from = &value["from"][0];
    let address = from["email"].as_str().unwrap_or_default().to_owned();
    let mut body = Vec::new();
    if let Some(parts) = value["textBody"].as_array() {
        for part in parts {
            if part["type"] != "text/plain" {
                continue;
            }
            let body_value = &value["bodyValues"][field(part, "partId")?];
            if body_value["isEncodingProblem"] == true {
                return Err(failed());
            }
            let mut text = field(body_value, "value")?.to_owned();
            if body_value["isTruncated"] == true {
                text.push_str("\n[Body truncated]");
            }
            body.push(text);
        }
    }
    Ok(Message {
        id: format!("jmap-{}", field(value, "id")?),
        sender: from["name"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or(&address)
            .to_owned(),
        address,
        subject: value["subject"].as_str().unwrap_or_default().to_owned(),
        date: value["receivedAt"].as_str().unwrap_or_default().to_owned(),
        body: if body.is_empty() {
            "This message has no plain-text part. Open it to read the HTML version.".into()
        } else {
            crate::bounded_body(body.join("\n"))
        },
    })
}

impl Drop for JmapAccount {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.token.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn queries_provider_sent_mail_and_text_filter_in_requested_order() {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let api = Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
        let server = std::thread::spawn(move || {
            for index in 0..3 {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut reader = BufReader::new(socket.try_clone().unwrap());
                let mut length = 0;
                loop {
                    let mut line = String::new();
                    reader.read_line(&mut line).unwrap();
                    if line == "\r\n" {
                        break;
                    }
                    if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse::<usize>().unwrap();
                    }
                }
                let mut bytes = vec![0; length];
                reader.read_exact(&mut bytes).unwrap();
                let request: Value = serde_json::from_slice(&bytes).unwrap();
                let call = &request["methodCalls"][0];
                assert_eq!(call[1]["accountId"], "account");
                let (method, response) = match index {
                    0 => (
                        "Mailbox/get",
                        json!({"list":[{"id":"sent-id","role":"sent"},{"id":"inbox-id","role":"inbox"}]}),
                    ),
                    1 => {
                        assert_eq!(
                            call[1]["filter"],
                            json!({"inMailbox":"sent-id","text":"needle"})
                        );
                        assert_eq!(call[1]["limit"], 100);
                        ("Email/query", json!({"ids":["b","a"]}))
                    }
                    _ => {
                        assert_eq!(call[1]["ids"], json!(["b", "a"]));
                        (
                            "Email/get",
                            json!({"list":[{"id":"a","subject":"Older"},{"id":"b","subject":"Newest"}]}),
                        )
                    }
                };
                assert_eq!(call[0], method);
                let body = json!({"methodResponses":[[method,response,"call"]]}).to_string();
                write!(
                    socket,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                )
                .unwrap();
            }
        });
        let account = JmapAccount {
            session_url: "https://example.test/jmap".into(),
            token: "secret".into(),
            address: "me@example.test".into(),
        };
        let session = Session {
            account: &account,
            client: Client::new(),
            api,
            download: String::new(),
            id: "account".into(),
            submission: false,
        };
        let messages = session.query("sent", "needle").unwrap();
        assert_eq!(
            messages
                .iter()
                .map(|m| m.subject.as_str())
                .collect::<Vec<_>>(),
            ["Newest", "Older"]
        );
        server.join().unwrap();
    }
    #[test]
    fn posts_authenticated_method_and_redacts_server_errors() {
        use std::{
            io::{BufRead, BufReader, Write},
            net::TcpListener,
            thread,
        };
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let api = Url::parse(&format!("http://{}", listener.local_addr().unwrap())).unwrap();
        let server = thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            let mut headers = String::new();
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some(value) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse::<usize>().unwrap();
                }
                headers.push_str(&line);
            }
            assert!(
                headers
                    .to_ascii_lowercase()
                    .contains("authorization: bearer test-token")
            );
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            let request: Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(
                request["methodCalls"][0],
                json!(["Identity/get",{"accountId":"account"},"call"])
            );
            assert!(
                request["using"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(SUBMISSION))
            );
            let body =
                r#"{"methodResponses":[["error",{"description":"private server data"},"call"]]}"#;
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        });
        let account = JmapAccount {
            session_url: "https://example.test".into(),
            token: "test-token".into(),
            address: "me@example.test".into(),
        };
        // Only this internal test bypasses HTTPS to exercise the wire format locally.
        let session = Session {
            account: &account,
            client: Client::new(),
            api,
            download: String::new(),
            id: "account".into(),
            submission: true,
        };
        let error = session
            .call("Identity/get", json!({}))
            .unwrap_err()
            .to_string();
        assert!(!error.contains("private server data"));
        assert!(!error.contains("test-token"));
        server.join().unwrap();
    }
    #[test]
    fn validates_transport_and_credentials() {
        assert!(secure_url("http://example.test/jmap").is_err());
        assert!(secure_url("https://user:secret@example.test/jmap").is_err());
        assert!(secure_url("https://example.test/jmap").is_ok());
        let account = JmapAccount {
            session_url: "https://example.test/jmap".into(),
            token: "bad\r\nheader".into(),
            address: "me@example.test".into(),
        };
        assert!(account.validate().is_err());
    }
    #[test]
    fn bounds_combined_text_parts() {
        let value = json!({"id":"large", "textBody":[{"partId":"one","type":"text/plain"},{"partId":"two","type":"text/plain"}], "bodyValues":{"one":{"value":"界".repeat(20_000)},"two":{"value":"界".repeat(20_000)}}});
        let message = parse_message(&value).unwrap();
        assert!(message.body.len() <= 100_100);
        assert!(message.body.ends_with("[Body truncated]"));
    }
    #[test]
    fn reads_plain_text_and_marks_truncation() {
        let value = json!({"id":"abc","from":[{"name":"Friend","email":"friend@example.test"}],
            "subject":"Hi 世界","textBody":[{"partId":"1","type":"text/plain"},{"partId":"image","type":"image/png"}],"bodyValues":{"1":{"value":"<script>text</script>","isTruncated":true}}});
        let mail = parse_message(&value).unwrap();
        assert_eq!(mail.id, "jmap-abc");
        assert_eq!(mail.sender, "Friend");
        assert_eq!(mail.body, "<script>text</script>\n[Body truncated]");
        assert!(parse_message(&json!({})).is_err());
        assert!(
            parse_message(&json!({"id":"html","textBody":[{"partId":"html","type":"text/html"}],"bodyValues":{"html":{"value":"<b>HTML</b>"}}}))
                .unwrap()
                .body
                .contains("no plain-text part")
        );
    }
    #[test]
    fn rejects_method_errors_and_wrong_call_ids() {
        assert!(
            method_result(
                &json!({"methodResponses":[["error",{"type":"forbidden"},"call"]]}),
                "Email/get"
            )
            .is_err()
        );
        assert!(
            method_result(
                &json!({"methodResponses":[["Email/get",{},"other"]]}),
                "Email/get"
            )
            .is_err()
        );
        assert!(
            method_result(
                &json!({"methodResponses":[["Email/get",{"list":[]},"call"]]}),
                "Email/get"
            )
            .is_ok()
        );
    }
}
