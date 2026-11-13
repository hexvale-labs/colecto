//! JMAP transport (RFC 8620, RFC 8621): receive, search, send with attachments and push.
//! Tokens and passwords remain in the trusted core.
use crate::{Error, mime::Attachment};
use colecto_extension_api::{DraftInput, Message};
use reqwest::{
    Url,
    blocking::{Client, RequestBuilder},
};
use serde_json::{Value, json};
use std::{io::Read, time::Duration};

const CORE: &str = "urn:ietf:params:jmap:core";
const MAIL: &str = "urn:ietf:params:jmap:mail";
const SUBMISSION: &str = "urn:ietf:params:jmap:submission";

#[derive(Clone)]
pub struct JmapAccount {
    /// The session resource, or just the server's HTTPS origin to use `/.well-known/jmap`.
    pub session_url: String,
    /// A bearer token, or the password when `username` is set.
    pub token: String,
    pub address: String,
    /// Empty for bearer tokens; otherwise the token is sent as this user's password (Basic auth).
    pub username: String,
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
/// HTTPS, or plain HTTP to a server on this machine, where nothing crosses the network.
fn secure_scheme(url: &Url) -> bool {
    url.scheme() == "https"
        || (url.scheme() == "http"
            && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")))
}
fn secure_url(value: &str) -> Result<Url, Error> {
    let url = Url::parse(value).map_err(|_| failed())?;
    if !secure_scheme(&url)
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
    {
        return Err(Error::Invalid(
            "JMAP requires an HTTPS URL (or HTTP on this machine) without embedded credentials or fragment".into(),
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

/// Follow at most a few redirects, and only to a secure URL on the same host, so credentials never
/// leave the configured server (`/.well-known/jmap` usually redirects to the real session).
fn same_host_redirects() -> reqwest::redirect::Policy {
    reqwest::redirect::Policy::custom(|attempt| {
        let host = attempt.previous().first().and_then(|url| url.host_str());
        if attempt.previous().len() <= 3
            && secure_scheme(attempt.url())
            && attempt.url().host_str() == host
        {
            attempt.follow()
        } else {
            attempt.stop()
        }
    })
}

struct Session<'a> {
    account: &'a JmapAccount,
    client: Client,
    api: Url,
    download: String,
    upload: String,
    max_upload: u64,
    id: String,
    submission: bool,
    event_source: Option<String>,
}
impl JmapAccount {
    fn authorize(&self, request: RequestBuilder) -> RequestBuilder {
        if self.username.is_empty() {
            request.bearer_auth(&self.token)
        } else {
            request.basic_auth(&self.username, Some(&self.token))
        }
    }
    fn session_resource(&self) -> Result<Url, Error> {
        let mut url = secure_url(&self.session_url)?;
        if url.path() == "/" && url.query().is_none() {
            url.set_path("/.well-known/jmap");
        }
        Ok(url)
    }
    pub fn validate(&self) -> Result<(), Error> {
        secure_url(&self.session_url)?;
        if self.token.trim().is_empty() || self.token.contains(['\r', '\n']) {
            return Err(Error::Invalid(
                "Configure a JMAP bearer token or password".into(),
            ));
        }
        if self.username.contains([':', '\r', '\n']) {
            return Err(Error::Invalid("Invalid JMAP username".into()));
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
            .redirect(same_host_redirects())
            .build()
            .map_err(|_| failed())?;
        let data = decode(
            self.authorize(client.get(self.session_resource()?))
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
            upload: data["uploadUrl"].as_str().unwrap_or_default().to_owned(),
            max_upload: data["capabilities"][CORE]["maxSizeUpload"]
                .as_u64()
                .unwrap_or(u64::MAX),
            submission: data["accounts"][&id]["accountCapabilities"][SUBMISSION].is_object(),
            event_source: data["eventSourceUrl"].as_str().map(str::to_owned),
            id,
        })
    }
    /// JMAP push (RFC 8620 §7.3): block until the account's Email state moves past the
    /// state seen on connect (`Ok(true)`), or until `timeout` passes (`Ok(false)`).
    pub(crate) fn wait_for_change(&self, timeout: Duration) -> Result<bool, Error> {
        let session = self.session()?;
        let template = session.event_source.as_deref().ok_or_else(|| {
            Error::Invalid("This JMAP server does not offer push notifications".into())
        })?;
        let baseline = session.call("Email/get", json!({"ids":[]}))?["state"]
            .as_str()
            .ok_or_else(failed)?
            .to_owned();
        let url = secure_url(
            &template
                .replace("{types}", "Email")
                .replace("{closeafter}", "no")
                .replace("{ping}", "60"),
        )?;
        let started = std::time::Instant::now();
        let client = Client::builder()
            .timeout(timeout)
            .redirect(same_host_redirects())
            .build()
            .map_err(|_| failed())?;
        let response = self
            .authorize(client.get(url))
            .header("Accept", "text/event-stream")
            .send()
            .map_err(http_error)?;
        if !response.status().is_success() {
            return Err(failed());
        }
        let mut reader = std::io::BufReader::new(response);
        let mut data = String::new();
        loop {
            let mut line = Vec::new();
            let read = std::io::BufRead::read_until(
                &mut std::io::Read::take(&mut reader, 65_537),
                b'\n',
                &mut line,
            );
            match read {
                // The request deadline ends a quiet stream; anything earlier is a dropped connection.
                Err(_) | Ok(0) if started.elapsed() + Duration::from_secs(1) >= timeout => {
                    return Ok(false);
                }
                Err(_) | Ok(0) => return Err(failed()),
                Ok(count) if count > 65_536 => return Err(failed()),
                Ok(_) => {}
            }
            let line = String::from_utf8_lossy(&line);
            let line = line.trim_end_matches(['\r', '\n']);
            if line.is_empty() {
                if email_state(&data, &session.id).is_some_and(|state| state != baseline) {
                    return Ok(true);
                }
                data.clear();
            } else if let Some(value) = line.strip_prefix("data:") {
                if data.len() + value.len() > 65_536 {
                    return Err(failed());
                }
                data.push_str(value.trim_start());
            }
        }
    }
    pub(crate) fn organize(
        &self,
        ids: &[String],
        seen: Option<bool>,
        flagged: Option<bool>,
        destination: Option<&str>,
        trash: Option<&str>,
        delete: bool,
    ) -> Result<(), Error> {
        let session = self.session()?;
        let mut update = serde_json::Map::new();
        let mut destroy = Vec::new();
        for id in ids {
            let id = id
                .strip_prefix("jmap-")
                .filter(|id| !id.is_empty())
                .ok_or_else(|| Error::Invalid("Invalid JMAP message id".into()))?;
            let mut patch = json!({});
            for (value, key) in [(seen, "keywords/$seen"), (flagged, "keywords/$flagged")] {
                if let Some(value) = value {
                    patch[key] = if value { json!(true) } else { Value::Null };
                }
            }
            let mut target = destination;
            if delete {
                let data = session.call(
                    "Email/get",
                    json!({"ids":[id],"properties":["id","mailboxIds"]}),
                )?;
                let email = list(&data)?.first().ok_or(Error::Missing)?;
                target = trash.filter(|trash| email["mailboxIds"][*trash] != true);
                if target.is_none() {
                    destroy.push(id);
                    continue;
                }
            }
            if let Some(target) = target {
                patch["mailboxIds"] = json!({target:true});
            }
            update.insert(id.into(), patch);
        }
        let result = session.call("Email/set", json!({"update":update,"destroy":destroy}))?;
        if result["notUpdated"]
            .as_object()
            .is_some_and(|v| !v.is_empty())
            || result["notDestroyed"]
                .as_object()
                .is_some_and(|v| !v.is_empty())
        {
            return Err(failed());
        }
        if update.keys().any(|id| {
            !result["updated"]
                .as_object()
                .is_some_and(|updated| updated.contains_key(id))
        }) || destroy.iter().any(|id| {
            !result["destroyed"]
                .as_array()
                .is_some_and(|destroyed| destroyed.iter().any(|value| value.as_str() == Some(id)))
        }) {
            return Err(failed());
        }
        Ok(())
    }

    pub fn folders(&self) -> Result<Vec<colecto_extension_api::Folder>, Error> {
        self.session()?.folders()
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
        session.download(field(email, "blobId")?)
    }
    /// Download several messages from one session with at most three downloads at a time.
    /// Messages over `max_size` bytes are skipped.
    pub(crate) fn fetch_many(
        &self,
        ids: &[String],
        max_size: usize,
        found: &mut dyn FnMut(String, Vec<u8>),
    ) -> Result<(), Error> {
        let wanted: Vec<&str> = ids
            .iter()
            .filter_map(|id| id.strip_prefix("jmap-"))
            .collect();
        if wanted.is_empty() {
            return Ok(());
        }
        let session = self.session()?;
        let data = session.call(
            "Email/get",
            json!({"ids":wanted,"properties":["id","blobId","size"]}),
        )?;
        let emails = list(&data)?;
        // Keep the caller's order (newest first) so likely reads arrive first.
        let blobs: Vec<(String, String)> = wanted
            .iter()
            .filter_map(|id| {
                let email = emails.iter().find(|email| email["id"] == *id)?;
                if email["size"].as_u64()? > max_size as u64 {
                    return None;
                }
                Some((format!("jmap-{id}"), email["blobId"].as_str()?.to_owned()))
            })
            .collect();
        let queue = std::sync::Mutex::new(blobs.into_iter());
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::scope(|scope| {
            for _ in 0..3 {
                let sender = sender.clone();
                let (queue, session) = (&queue, &session);
                scope.spawn(move || {
                    while let Some((id, blob)) = queue.lock().ok().and_then(|mut q| q.next()) {
                        if sender.send((id, session.download(&blob))).is_err() {
                            return;
                        }
                    }
                });
            }
            drop(sender);
            for (id, raw) in receiver {
                if let Ok(raw) = raw {
                    found(id, raw);
                }
            }
        });
        Ok(())
    }
    pub(crate) fn send(&self, input: &DraftInput, attachments: &[Attachment]) -> Result<(), Error> {
        self.send_inner(input, attachments).map_err(|error| match error {
            Error::Transport(message) => Error::Transport(format!("{message} Sending may have an uncertain outcome; check Sent and delivery status before sending again.")),
            error => error,
        })
    }
    fn send_inner(&self, input: &DraftInput, attachments: &[Attachment]) -> Result<(), Error> {
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
        if !attachments.is_empty() {
            email["attachments"] = attachments
                .iter()
                .map(|file| {
                    Ok(
                        json!({"blobId": session.upload(file)?, "type": file.content_type,
                        "name": file.name, "disposition": "attachment"}),
                    )
                })
                .collect::<Result<Vec<_>, Error>>()?
                .into();
        }
        let identities = session.call("Identity/get", json!({}))?;
        let identity = identity_for(list(&identities)?, &self.address).ok_or_else(|| {
            Error::Invalid(
                "This JMAP account has no sending identity for your email address".into(),
            )
        })?;
        let drafts = session.mailbox("drafts")?;
        let sent = session.mailbox("sent")?;
        email["mailboxIds"] = json!({(drafts.clone()): true});
        email["keywords"] = json!({"$draft": true});
        let created = session.call("Email/set", json!({"create":{"message":email}}))?;
        let email = created_id(&created, "message")?;
        let result = session.call("EmailSubmission/set", json!({"create":{"submission":{
            "identityId":field(identity,"id")?,"emailId":email}},
            "onSuccessUpdateEmail":{"#submission":{
                (format!("mailboxIds/{drafts}")):null,(format!("mailboxIds/{sent}")):true,"keywords/$draft":null
            }}}))?;
        created_id(&result, "submission")?;
        Ok(())
    }
}
impl Session<'_> {
    fn folders(&self) -> Result<Vec<colecto_extension_api::Folder>, Error> {
        let data = self.call(
            "Mailbox/get",
            json!({
                "properties":["id","name","role","totalEmails","unreadEmails"]
            }),
        )?;
        list(&data)?
            .iter()
            .map(|mailbox| {
                Ok(colecto_extension_api::Folder {
                    id: field(mailbox, "id")?.into(),
                    name: field(mailbox, "name")?.into(),
                    role: serde_json::from_value(mailbox["role"].clone())
                        .unwrap_or(colecto_extension_api::FolderRole::Other),
                    total: mailbox["totalEmails"]
                        .as_u64()
                        .and_then(|n| n.try_into().ok())
                        .ok_or_else(failed)?,
                    unread: mailbox["unreadEmails"]
                        .as_u64()
                        .and_then(|n| n.try_into().ok())
                        .ok_or_else(failed)?,
                })
            })
            .collect()
    }

    fn query(&self, folder: &str, search: &str) -> Result<Vec<Message>, Error> {
        let mailbox = self.mailbox(folder)?;
        let mut filter = json!({"inMailbox":mailbox});
        if !search.is_empty() {
            filter["text"] = json!(search);
        }
        // shortcut: latest 100 messages only; add changes/pagination before full sync.
        // One request: Email/get takes the ids straight from the query's result.
        let results = self.calls(&[
            (
                "Email/query",
                json!({"filter":filter,
                "sort":[{"property":"receivedAt","isAscending":false}],"limit":crate::MAX_LIST_MESSAGES}),
            ),
            (
                "Email/get",
                json!({"#ids":{"resultOf":"0","name":"Email/query","path":"/ids"},
                "properties":["id","from","subject","receivedAt","textBody","bodyValues","keywords"],
                "fetchTextBodyValues":true,"maxBodyValueBytes":100000}),
            ),
        ])?;
        let ids = results[0]["ids"].as_array().ok_or_else(failed)?;
        let messages = list(&results[1])?;
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

    fn call(&self, method: &str, args: Value) -> Result<Value, Error> {
        Ok(self.calls(&[(method, args)])?.remove(0))
    }
    /// Send several method calls in one request; call ids are their positions.
    fn calls(&self, calls: &[(&str, Value)]) -> Result<Vec<Value>, Error> {
        let mut using = vec![CORE, MAIL];
        if calls.iter().any(|(method, _)| {
            method.starts_with("Identity/") || method.starts_with("EmailSubmission/")
        }) {
            using.push(SUBMISSION);
        }
        let method_calls = calls
            .iter()
            .enumerate()
            .map(|(index, (method, args))| {
                let mut args = args.clone();
                args["accountId"] = json!(self.id);
                json!([method, args, index.to_string()])
            })
            .collect::<Vec<_>>();
        let data = decode(
            self.account
                .authorize(self.client.post(self.api.clone()))
                .json(&json!({"using":using,"methodCalls":method_calls}))
                .send()
                .map_err(http_error)?,
        )?;
        calls
            .iter()
            .enumerate()
            .map(|(index, (method, _))| method_result(&data, method, &index.to_string()))
            .collect()
    }
    /// Download one blob as RFC 5322 bytes, bounded by the largest message Colecto opens.
    fn download(&self, blob: &str) -> Result<Vec<u8>, Error> {
        // JMAP Ids are URL-safe; refuse anything else rather than escaping it into a URL.
        if [blob, self.id.as_str()].iter().any(|value| {
            value.is_empty()
                || !value
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
        }) {
            return Err(failed());
        }
        let url = self
            .download
            .replace("{accountId}", &self.id)
            .replace("{blobId}", blob)
            .replace("{type}", "message%2Frfc822")
            .replace("{name}", "message.eml");
        let response = self
            .account
            .authorize(self.client.get(secure_url(&url)?))
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
    /// Upload one attachment and return its blob id (RFC 8620 §6.1).
    fn upload(&self, file: &Attachment) -> Result<String, Error> {
        if file.data.len() as u64 > self.max_upload {
            return Err(Error::Invalid(format!(
                "{} is larger than this JMAP server accepts",
                file.name
            )));
        }
        let url = self.upload.replace("{accountId}", &self.id);
        let data = decode(
            self.account
                .authorize(self.client.post(secure_url(&url)?))
                .header(reqwest::header::CONTENT_TYPE, &file.content_type)
                .body(file.data.clone())
                .send()
                .map_err(http_error)?,
        )?;
        Ok(field(&data, "blobId")?.to_owned())
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
/// The identity to send as: an exact address match, then a `*@domain` wildcard identity,
/// then the account's only identity (some servers leave its address empty).
fn identity_for<'a>(identities: &'a [Value], address: &str) -> Option<&'a Value> {
    fn email<'v>(identity: &&'v Value) -> &'v str {
        identity["email"].as_str().unwrap_or_default()
    }
    let domain = address.rsplit_once('@').map(|(_, domain)| domain)?;
    identities
        .iter()
        .find(|identity| email(identity).eq_ignore_ascii_case(address))
        .or_else(|| {
            identities.iter().find(|identity| {
                email(identity)
                    .strip_prefix("*@")
                    .is_some_and(|wildcard| wildcard.eq_ignore_ascii_case(domain))
            })
        })
        .or(match identities {
            [only] => Some(only),
            _ => None,
        })
}
/// The server id of an object created by a `/set` call, or why the server refused it.
fn created_id<'a>(result: &'a Value, key: &str) -> Result<&'a str, Error> {
    if let Some(kind) = result["notCreated"][key]["type"].as_str() {
        let reason = match kind {
            "tooLarge" => "tooLarge: message exceeds provider limits",
            "overQuota" => "overQuota: provider storage is full",
            "forbidden" => "forbidden: provider denied permission",
            "invalidProperties" => "invalidProperties: provider rejected message fields",
            _ => "provider rejected creation",
        };
        return Err(Error::Transport(format!(
            "The JMAP server refused the message ({reason})."
        )));
    }
    field(&result["created"][key], "id")
}
/// The Email state for `account` in a JMAP StateChange event, if it names one.
fn email_state(data: &str, account: &str) -> Option<String> {
    let value: Value = serde_json::from_str(data).ok()?;
    (value["@type"] == "StateChange")
        .then(|| {
            value["changed"][account]["Email"]
                .as_str()
                .map(str::to_owned)
        })
        .flatten()
}
fn method_result(data: &Value, method: &str, call: &str) -> Result<Value, Error> {
    let responses = data["methodResponses"].as_array().ok_or_else(failed)?;
    if let Some(error) = responses.iter().find(|v| v[0] == "error" && v[2] == call) {
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
        .find(|v| v[0] == method && v[2] == call)
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
        seen: value["keywords"]["$seen"].as_bool().unwrap_or(false),
        flagged: value["keywords"]["$flagged"].as_bool().unwrap_or(false),
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
    fn creation_errors_do_not_echo_provider_text() {
        let error = created_id(&json!({"notCreated":{"x":{"type":"secret\ntext"}}}), "x")
            .unwrap_err()
            .to_string();
        assert!(!error.contains("secret"));
        assert!(error.contains("provider rejected creation"));
    }
    #[test]
    fn parses_seen_and_flagged_keywords() {
        let message =
            parse_message(&json!({"id":"a","keywords":{"$seen":true,"$flagged":true}})).unwrap();
        assert!(message.seen && message.flagged);
        let message = parse_message(&json!({"id":"a"})).unwrap();
        assert!(!message.seen && !message.flagged);
    }
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
                let calls = request["methodCalls"].as_array().unwrap();
                assert!(calls.iter().all(|call| call[1]["accountId"] == "account"));
                let responses = if index == 2 {
                    assert_eq!(calls[0][0], "Mailbox/get");
                    assert!(
                        calls[0][1]["properties"]
                            .as_array()
                            .unwrap()
                            .contains(&json!("unreadEmails"))
                    );
                    json!([["Mailbox/get",{"list":[
                        {"id":"in","name":"Inbox","role":"inbox","totalEmails":14,"unreadEmails":3},
                        {"id":"custom","name":"Projects/2026","role":null,"totalEmails":8,"unreadEmails":0}
                    ]},"0"]])
                } else if index == 0 {
                    assert_eq!(calls[0][0], "Mailbox/get");
                    json!([["Mailbox/get",{"list":[{"id":"sent-id","role":"sent"},{"id":"inbox-id","role":"inbox"}]},"0"]])
                } else {
                    // The query and the fetch travel together, linked by a result reference.
                    assert_eq!(calls[0][0], "Email/query");
                    assert_eq!(
                        calls[0][1]["filter"],
                        json!({"inMailbox":"sent-id","text":"needle"})
                    );
                    assert_eq!(calls[0][1]["limit"], 100);
                    assert_eq!(calls[1][0], "Email/get");
                    assert_eq!(calls[1][1]["#ids"]["resultOf"], "0");
                    json!([
                        ["Email/query",{"ids":["b","a"]},"0"],
                        ["Email/get",{"list":[{"id":"a","subject":"Older"},{"id":"b","subject":"Newest"}]},"1"]
                    ])
                };
                let body = json!({"methodResponses":responses}).to_string();
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
            username: String::new(),
        };
        let session = Session {
            account: &account,
            client: Client::new(),
            api,
            download: String::new(),
            upload: String::new(),
            max_upload: u64::MAX,
            id: "account".into(),
            submission: false,
            event_source: None,
        };
        let messages = session.query("sent", "needle").unwrap();
        assert_eq!(
            messages
                .iter()
                .map(|m| m.subject.as_str())
                .collect::<Vec<_>>(),
            ["Newest", "Older"]
        );
        let folders = session.folders().unwrap();
        assert_eq!(folders[0].role, colecto_extension_api::FolderRole::Inbox);
        assert_eq!((folders[0].total, folders[0].unread), (14, 3));
        assert_eq!(folders[1].id, "custom");
        assert_eq!(folders[1].role, colecto_extension_api::FolderRole::Other);
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
                json!(["Identity/get",{"accountId":"account"},"0"])
            );
            assert!(
                request["using"]
                    .as_array()
                    .unwrap()
                    .contains(&json!(SUBMISSION))
            );
            let body =
                r#"{"methodResponses":[["error",{"description":"private server data"},"0"]]}"#;
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
            username: String::new(),
        };
        // Only this internal test bypasses HTTPS to exercise the wire format locally.
        let session = Session {
            account: &account,
            client: Client::new(),
            api,
            download: String::new(),
            upload: String::new(),
            max_upload: u64::MAX,
            id: "account".into(),
            submission: true,
            event_source: None,
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
        assert!(secure_url("http://localhost:8080/jmap").is_ok());
        assert!(secure_url("http://127.0.0.1/jmap").is_ok());
        assert!(secure_url("http://localhost.example.test/jmap").is_err());
        assert!(secure_url("https://user:secret@example.test/jmap").is_err());
        assert!(secure_url("https://example.test/jmap").is_ok());
        let mut account = JmapAccount {
            session_url: "https://example.test".into(),
            token: "secret".into(),
            address: "me@example.test".into(),
            username: String::new(),
        };
        assert_eq!(
            account.session_resource().unwrap().as_str(),
            "https://example.test/.well-known/jmap"
        );
        account.session_url = "https://example.test/jmap/session".into();
        assert_eq!(account.session_resource().unwrap().path(), "/jmap/session");
        account.username = "me:you".into();
        assert!(account.validate().is_err());
        account.username = "me@example.test".into();
        assert!(account.validate().is_ok());
        assert!(
            created_id(
                &json!({"notCreated":{"message":{"type":"tooLarge"}}}),
                "message"
            )
            .unwrap_err()
            .to_string()
            .contains("tooLarge")
        );
        let account = JmapAccount {
            session_url: "https://example.test/jmap".into(),
            token: "bad\r\nheader".into(),
            address: "me@example.test".into(),
            username: String::new(),
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
                &json!({"methodResponses":[["error",{"type":"forbidden"},"0"]]}),
                "Email/get",
                "0"
            )
            .is_err()
        );
        assert!(
            method_result(
                &json!({"methodResponses":[["Email/get",{},"other"]]}),
                "Email/get",
                "0"
            )
            .is_err()
        );
        assert!(
            method_result(
                &json!({"methodResponses":[["Email/get",{"list":[]},"0"]]}),
                "Email/get",
                "0"
            )
            .is_ok()
        );
    }
    #[test]
    fn picks_the_matching_wildcard_or_only_identity() {
        let pick = |identities: Value| {
            identity_for(identities.as_array().unwrap(), "Me@Example.test")
                .map(|identity| identity["id"].as_str().unwrap().to_owned())
        };
        let exact = json!([{"id":"other","email":"other@example.test"},{"id":"me","email":"me@example.test"}]);
        assert_eq!(pick(exact).as_deref(), Some("me"));
        let wildcard = json!([{"id":"other","email":"x@elsewhere.test"},{"id":"any","email":"*@example.test"}]);
        assert_eq!(pick(wildcard).as_deref(), Some("any"));
        assert_eq!(
            pick(json!([{"id":"only","email":""}])).as_deref(),
            Some("only")
        );
        assert_eq!(
            pick(json!([{"id":"a","email":"a@x.test"},{"id":"b","email":"b@x.test"}])),
            None
        );
    }
    #[test]
    fn push_events_report_only_email_state_for_this_account() {
        let event = r#"{"@type":"StateChange","changed":{"a1":{"Email":"s2","Mailbox":"m1"}}}"#;
        assert_eq!(email_state(event, "a1").as_deref(), Some("s2"));
        assert_eq!(email_state(event, "other"), None);
        assert_eq!(
            email_state(
                r#"{"@type":"StateChange","changed":{"a1":{"Mailbox":"m2"}}}"#,
                "a1"
            ),
            None
        );
        assert_eq!(email_state(r#"{"@type":"Ping"}"#, "a1"), None);
        assert_eq!(email_state("not json", "a1"), None);
    }
}
