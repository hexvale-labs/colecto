//! Single-account TLS mail transports. Credentials stay in the trusted core.
use crate::{Error, validate};
use colecto_extension_api::{DraftInput, Message};
use lettre::{
    SmtpTransport, Transport, message::header::ContentType,
    transport::smtp::authentication::Credentials,
};
use mailparse::MailHeaderMap;
use std::{
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};

pub struct MailAccount {
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub username: String,
    pub password: String,
    pub address: String,
}

fn remote(_: impl std::fmt::Display) -> Error {
    Error::Transport(
        "Mail server operation failed. Check server settings and connection, then refresh again."
            .into(),
    )
}
fn imap_error(error: imap::error::Error) -> Error {
    let message = match error {
        imap::error::Error::No(_) => {
            "Mail server rejected the request (IMAP NO). Check mailbox access and account settings."
        }
        imap::error::Error::Bad(_) => {
            "Mail server rejected the request (IMAP BAD). Check server compatibility and settings."
        }
        imap::error::Error::Io(ref error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
            ) =>
        {
            "Mail server timed out. Your cached mail is still available; try refreshing again."
        }
        _ => return remote(error),
    };
    Error::Transport(message.into())
}
fn smtp_error(error: lettre::transport::smtp::Error) -> Error {
    let detail = if let Some(status) = error.status() {
        format!("SMTP {}", u16::from(status))
    } else if error.is_timeout() {
        "SMTP timeout".into()
    } else if error.is_tls() {
        "SMTP TLS negotiation failed".into()
    } else {
        "SMTP connection failed".into()
    };
    Error::Transport(format!(
        "Mail delivery failed ({detail}) and may have an uncertain outcome; check Sent and delivery status before sending again."
    ))
}

impl MailAccount {
    pub fn validate(&self) -> Result<(), Error> {
        if self.imap_host.is_empty()
            || self.smtp_host.is_empty()
            || self.username.is_empty()
            || self.password.is_empty()
            || self.imap_port == 0
            || !matches!(self.smtp_port, 465 | 587)
        {
            return Err(Error::Invalid(
                "Configure IMAP and SMTP hosts, credentials, and SMTP port 465 or 587".into(),
            ));
        }
        self.address
            .parse::<lettre::message::Mailbox>()
            .map_err(|_| Error::Invalid("Invalid sender address".into()))?;
        Ok(())
    }

    pub fn query(&self, folder: &str, search: &str) -> Result<Vec<Message>, Error> {
        self.query_with_identity(folder, search)
            .map(|(messages, _)| messages)
    }
    pub(crate) fn query_with_identity(
        &self,
        folder: &str,
        search: &str,
    ) -> Result<(Vec<Message>, String), Error> {
        self.validate()?;
        crate::validate_query(folder, search)?;
        let timeout = Duration::from_secs(30);
        let addresses = (self.imap_host.as_str(), self.imap_port)
            .to_socket_addrs()
            .map_err(remote)?;
        let mut socket = None;
        for address in addresses {
            if let Ok(stream) = TcpStream::connect_timeout(&address, timeout) {
                socket = Some(stream);
                break;
            }
        }
        let socket = socket.ok_or_else(|| remote("connect"))?;
        socket.set_read_timeout(Some(timeout)).map_err(remote)?;
        socket.set_write_timeout(Some(timeout)).map_err(remote)?;
        let tls = native_tls::TlsConnector::new().map_err(remote)?;
        let stream = tls.connect(&self.imap_host, socket).map_err(|_| {
            Error::Transport(
                "Mail server TLS negotiation failed. Check the server name and certificate.".into(),
            )
        })?;
        let mut client = imap::Client::new(stream);
        client.read_greeting().map_err(imap_error)?;
        let mut session = client
            .login(&self.username, &self.password)
            .map_err(|(error, _)| match error {
                imap::error::Error::No(_) => Error::Transport(
                    "Mail server rejected authentication. Check your username and password.".into(),
                ),
                error => imap_error(error),
            })?;
        let result = query_session_with_identity(&mut session, folder, search);
        let _ = session.logout();
        result
    }

    pub fn send(&self, input: &DraftInput) -> Result<(), Error> {
        self.validate()?;
        validate(&input.recipient, &input.subject, &input.body)?;
        let email = lettre::Message::builder()
            .from(
                self.address
                    .parse()
                    .map_err(|_| Error::Invalid("Invalid sender address".into()))?,
            )
            .to(input
                .recipient
                .parse()
                .map_err(|_| Error::Invalid("Enter one valid recipient email address".into()))?)
            .subject(&input.subject)
            .header(ContentType::TEXT_PLAIN)
            .body(input.body.clone())
            .map_err(|_| Error::Invalid("Invalid outgoing message".into()))?;
        let builder = if self.smtp_port == 465 {
            SmtpTransport::relay(&self.smtp_host)
        } else {
            SmtpTransport::starttls_relay(&self.smtp_host)
        }
        .map_err(remote)?;
        builder
            .port(self.smtp_port)
            .timeout(Some(Duration::from_secs(30)))
            .credentials(Credentials::new(
                self.username.clone(),
                self.password.clone(),
            ))
            .build()
            .send(&email)
            .map_err(smtp_error)?;
        Ok(())
    }
}

#[cfg(test)]
fn query_session<T: std::io::Read + std::io::Write>(
    session: &mut imap::Session<T>,
    folder: &str,
    search: &str,
) -> Result<Vec<Message>, Error> {
    query_session_with_identity(session, folder, search).map(|(messages, _)| messages)
}
fn query_session_with_identity<T: std::io::Read + std::io::Write>(
    session: &mut imap::Session<T>,
    folder: &str,
    search: &str,
) -> Result<(Vec<Message>, String), Error> {
    let name = if folder == "inbox" {
        "INBOX".to_owned()
    } else {
        let names = session.list(Some(""), Some("*")).map_err(imap_error)?;
        sent_mailbox(&names)?
    };
    let mailbox = session.examine(&name).map_err(imap_error)?;
    let validity = mailbox
        .uid_validity
        .ok_or_else(|| remote("Missing UIDVALIDITY"))?;
    let identity = if folder == "inbox" {
        format!("imap-{validity}-")
    } else {
        let namespace: String = name.bytes().map(|byte| format!("{byte:02x}")).collect();
        format!("imap-{namespace}-{validity}-")
    };
    if mailbox.exists == 0 {
        return Ok((Vec::new(), identity));
    }
    let headers = if search.is_empty() {
        let range = format!("{}:*", mailbox.exists.saturating_sub(99).max(1));
        session
            .fetch(&range, "(UID RFC822.SIZE)")
            .map_err(imap_error)?
    } else {
        let mut ids: Vec<_> = session
            .uid_search(search_expression(search))
            .map_err(imap_error)?
            .into_iter()
            .collect();
        ids.sort_unstable();
        let selected: Vec<_> = ids
            .into_iter()
            .rev()
            .take(100)
            .map(|id| id.to_string())
            .collect();
        if selected.is_empty() {
            return Ok((Vec::new(), identity));
        }
        session
            .uid_fetch(selected.join(","), "(UID RFC822.SIZE)")
            .map_err(imap_error)?
    };
    let mut messages = Vec::new();
    for header in headers.iter().rev() {
        let uid = header.uid.ok_or_else(|| remote("Missing UID"))?;
        if header.size.unwrap_or(u32::MAX) > 2_000_000 {
            continue;
        }
        let fetched = session
            .uid_fetch(uid.to_string(), "(UID BODY.PEEK[])")
            .map_err(imap_error)?;
        for item in fetched.iter() {
            let raw = item.body().ok_or_else(|| remote("Missing body"))?;
            let mut message = parse_message(raw, validity, uid)?;
            if folder != "inbox" {
                let namespace: String = name.bytes().map(|byte| format!("{byte:02x}")).collect();
                message.id = format!("imap-{namespace}-{validity}-{uid}");
            }
            messages.push(message);
        }
    }
    Ok((messages, identity))
}

fn search_expression(search: &str) -> String {
    let escaped = search.replace('\\', "\\\\").replace('"', "\\\"");
    let charset = if search.is_ascii() {
        ""
    } else {
        "CHARSET UTF-8 "
    };
    format!("{charset}TEXT \"{escaped}\"")
}
fn sent_mailbox(names: &[imap::types::Name]) -> Result<String, Error> {
    let selectable = |name: &&imap::types::Name| {
        !name
            .attributes()
            .contains(&imap::types::NameAttribute::NoSelect)
    };
    names.iter().filter(selectable).find(|name| name.attributes().iter().any(|attribute| matches!(attribute, imap::types::NameAttribute::Custom(value) if value.eq_ignore_ascii_case("\\Sent"))))
        .or_else(|| names.iter().filter(selectable).find(|name| ["Sent", "Sent Items", "Sent Messages", "[Gmail]/Sent Mail"].iter().any(|candidate| name.name().eq_ignore_ascii_case(candidate))))
        .map(|name| name.name().to_owned())
        .ok_or_else(|| Error::Invalid("The provider did not advertise a Sent mailbox and no standard Sent folder was found".into()))
}

fn text_body(mail: &mailparse::ParsedMail<'_>) -> Result<String, Error> {
    if mail.ctype.mimetype == "text/plain"
        && mail.get_content_disposition().disposition != mailparse::DispositionType::Attachment
    {
        return mail.get_body().map_err(remote);
    }
    for part in &mail.subparts {
        if part.get_content_disposition().disposition == mailparse::DispositionType::Attachment {
            continue;
        }
        let text = text_body(part)?;
        if !text.is_empty() {
            return Ok(text);
        }
    }
    Ok(String::new())
}

fn parse_message(raw: &[u8], validity: u32, uid: u32) -> Result<Message, Error> {
    let mail = mailparse::parse_mail(raw).map_err(remote)?;
    let from = mail.headers.get_first_value("From").unwrap_or_default();
    let body = crate::bounded_body(text_body(&mail)?);
    Ok(Message {
        id: format!("imap-{validity}-{uid}"),
        sender: from.clone(),
        address: from,
        subject: mail.headers.get_first_value("Subject").unwrap_or_default(),
        date: mail.headers.get_first_value("Date").unwrap_or_default(),
        body: if body.is_empty() {
            "No plain-text body available. HTML and attachments are not displayed.".into()
        } else {
            body
        },
    })
}

impl Drop for MailAccount {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        self.password.zeroize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn empty_mailbox_reports_successful_uidvalidity_changes() {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            write!(socket, "* OK ready\r\n").unwrap();
            for (expected, response) in [
                ("LOGIN", String::new()),
                (
                    "EXAMINE",
                    "* 0 EXISTS\r\n* OK [UIDVALIDITY 7] stable\r\n".into(),
                ),
                (
                    "EXAMINE",
                    "* 0 EXISTS\r\n* OK [UIDVALIDITY 8] changed\r\n".into(),
                ),
            ] {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let (tag, command) = line.split_once(' ').unwrap();
                assert!(command.starts_with(expected));
                write!(socket, "{response}{tag} OK complete\r\n").unwrap();
            }
        });
        let mut client = imap::Client::new(TcpStream::connect(address).unwrap());
        client.read_greeting().unwrap();
        let mut session = client.login("me", "secret").unwrap();
        for (search, identity) in [("", "imap-7-"), ("needle", "imap-8-")] {
            let result = query_session_with_identity(&mut session, "inbox", search).unwrap();
            assert!(result.0.is_empty());
            assert_eq!(result.1, identity);
        }
        server.join().unwrap();
    }
    #[test]
    fn server_status_errors_never_include_protocol_text() {
        for (error, expected) in [
            (
                imap::error::Error::No("password=secret private mail".into()),
                "IMAP NO",
            ),
            (imap::error::Error::Bad("private mail".into()), "IMAP BAD"),
        ] {
            let message = imap_error(error).to_string();
            assert!(message.contains(expected));
            assert!(!message.contains("secret"));
            assert!(!message.contains("private mail"));
        }
    }
    #[test]
    fn uses_provider_sent_role_and_search_without_marking_mail_seen() {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            write!(socket, "* OK ready\r\n").unwrap();
            let raw = "From: me@example.test\r\nSubject: Needle\r\n\r\nFound it";
            let steps = [
                ("LOGIN", String::new()),
                (
                    "LIST",
                    "* LIST (\\Sent) \"/\" \"Outbox archive\"\r\n".into(),
                ),
                (
                    "EXAMINE \"Outbox archive\"",
                    "* 1 EXISTS\r\n* OK [UIDVALIDITY 7] stable\r\n".into(),
                ),
                ("UID SEARCH TEXT \"needle\"", "* SEARCH 42\r\n".into()),
                (
                    "UID FETCH 42 (UID RFC822.SIZE)",
                    "* 1 FETCH (UID 42 RFC822.SIZE 80)\r\n".into(),
                ),
                (
                    "UID FETCH 42 (UID BODY.PEEK[])",
                    format!("* 1 FETCH (UID 42 BODY[] {{{}}}\r\n{})\r\n", raw.len(), raw),
                ),
            ];
            for (expected, response) in steps {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let (tag, command) = line.split_once(' ').unwrap();
                assert!(
                    command.starts_with(expected),
                    "unexpected command: {command}"
                );
                write!(socket, "{response}{tag} OK complete\r\n").unwrap();
            }
        });
        let mut client = imap::Client::new(TcpStream::connect(address).unwrap());
        client.read_greeting().unwrap();
        let mut session = client.login("me", "secret").unwrap();
        let messages = query_session(&mut session, "sent", "needle").unwrap();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].subject, "Needle");
        assert_eq!(messages[0].id, "imap-4f7574626f782061726368697665-7-42");
        server.join().unwrap();
    }
    #[test]
    fn search_quotes_untrusted_text_and_bounds_decoded_bodies() {
        assert_eq!(search_expression("a\\b\"c"), r#"TEXT "a\\b\"c""#);
        assert_eq!(search_expression("世界"), "CHARSET UTF-8 TEXT \"世界\"");
        let raw = format!(
            "Content-Type: text/plain; charset=utf-8\r\n\r\n{}",
            "界".repeat(40_000)
        );
        let message = parse_message(raw.as_bytes(), 1, 1).unwrap();
        assert!(message.body.len() <= 100_100);
        assert!(message.body.ends_with("[Body truncated]"));
    }
    #[test]
    fn decodes_mime_without_rendering_html_or_attachments() {
        let raw = b"From: Friend <friend@example.test>\r\nSubject: =?UTF-8?Q?Hello_=E4=B8=96=E7=95=8C?=\r\nContent-Type: multipart/alternative; boundary=x\r\n\r\n--x\r\nContent-Type: text/html\r\n\r\n<script>bad()</script>\r\n--x\r\nContent-Type: text/plain; charset=utf-8\r\nContent-Transfer-Encoding: base64\r\n\r\nSGVsbG8=\r\n--x--\r\n";
        let message = parse_message(raw, 8, 12).unwrap();
        assert_eq!(message.id, "imap-8-12");
        assert_eq!(message.subject, "Hello 世界");
        assert_eq!(message.body, "Hello");
    }
    #[test]
    fn rejects_insecure_smtp_and_invalid_recipient_before_connecting() {
        let mut account = MailAccount {
            imap_host: "localhost".into(),
            imap_port: 993,
            smtp_host: "localhost".into(),
            smtp_port: 25,
            username: "user".into(),
            password: "secret".into(),
            address: "me@example.test".into(),
        };
        assert!(matches!(account.validate(), Err(Error::Invalid(_))));
        account.smtp_port = 587;
        assert!(matches!(
            account.send(&DraftInput {
                id: None,
                recipient: "bad\r\nBcc: victim@example.test".into(),
                subject: "Hi".into(),
                body: "Hello".into()
            }),
            Err(Error::Invalid(_))
        ));
    }
}
