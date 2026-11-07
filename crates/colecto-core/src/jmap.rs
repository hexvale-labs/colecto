//! Prototype JMAP transport. Tokens remain in the trusted core.
use crate::{Error, validate};
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
    Error::Transport("JMAP operation failed. Check settings and credentials. Sending may have an uncertain outcome; check Sent and Drafts before retrying.".into())
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
    let response = response.error_for_status().map_err(|_| failed())?;
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
                .map_err(|_| failed())?,
        )?;
        let id = field(&data["primaryAccounts"], MAIL)?.to_owned();
        if !data["accounts"][&id]["accountCapabilities"][MAIL].is_object() {
            return Err(failed());
        }
        Ok(Session {
            account: self,
            client,
            api: secure_url(field(&data, "apiUrl")?)?,
            submission: data["accounts"][&id]["accountCapabilities"][SUBMISSION].is_object(),
            id,
        })
    }
    pub fn receive(&self) -> Result<Vec<Message>, Error> {
        let session = self.session()?;
        let inbox = session.mailbox("inbox")?;
        // shortcut: latest 100 messages only; add changes/pagination before full sync.
        let query = session.call(
            "Email/query",
            json!({"filter":{"inMailbox":inbox},
            "sort":[{"property":"receivedAt","isAscending":false}],"limit":100}),
        )?;
        let ids = query["ids"].as_array().ok_or_else(failed)?;
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let data = session.call(
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
    pub fn send(&self, input: &DraftInput) -> Result<(), Error> {
        self.validate()?;
        validate(&input.recipient, &input.subject, &input.body)?;
        input
            .recipient
            .parse::<lettre::Address>()
            .map_err(|_| Error::Invalid("Enter one valid recipient email address".into()))?;
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
        let created = session.call("Email/set", json!({"create":{"message":{
            "mailboxIds":{(drafts.clone()):true}, "keywords":{"$draft":true},
            "from":[{"email":self.address}],"to":[{"email":input.recipient}],"subject":input.subject,
            "textBody":[{"partId":"text","type":"text/plain"}],"bodyValues":{"text":{"value":input.body}}
        }}}))?;
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
                .map_err(|_| failed())?,
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
            "No plain-text body available. HTML and attachments are not displayed.".into()
        } else {
            body.join("\n")
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn reads_plain_text_and_marks_truncation() {
        let value = json!({"id":"abc","from":[{"name":"Friend","email":"friend@example.test"}],
            "subject":"Hi 世界","textBody":[{"partId":"1"}],"bodyValues":{"1":{"value":"<script>text</script>","isTruncated":true}}});
        let mail = parse_message(&value).unwrap();
        assert_eq!(mail.id, "jmap-abc");
        assert_eq!(mail.sender, "Friend");
        assert_eq!(mail.body, "<script>text</script>\n[Body truncated]");
        assert!(parse_message(&json!({})).is_err());
        assert!(
            parse_message(&json!({"id":"html","textBody":[]}))
                .unwrap()
                .body
                .contains("No plain-text")
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
