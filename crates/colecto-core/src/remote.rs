//! Single-account TLS mail transports. Credentials stay in the trusted core.
use crate::{Error, mime::Attachment};
use colecto_extension_api::{DraftInput, Message};
use lettre::{
    SmtpTransport, Transport,
    message::{MultiPart, SinglePart, header::ContentType},
    transport::smtp::{
        authentication::Credentials,
        client::{Certificate, CertificateStore, Tls, TlsParameters},
    },
};
use mailparse::MailHeaderMap;
use std::{
    net::{TcpStream, ToSocketAddrs},
    time::Duration,
};

#[derive(Clone)]
pub struct MailAccount {
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub username: String,
    pub password: String,
    pub address: String,
    /// Upgrade a plain IMAP connection with STARTTLS instead of starting with TLS.
    pub imap_starttls: bool,
    /// A PEM certificate to trust instead of the system's authorities, for IMAP and SMTP.
    pub tls_certificate: String,
}

/// Largest accepted pinned certificate, in bytes of PEM text.
pub const MAX_CERTIFICATE_BYTES: usize = 16 * 1024;

fn invalid_certificate() -> Error {
    Error::Invalid(
        "The trusted certificate must be one PEM certificate (BEGIN CERTIFICATE).".into(),
    )
}
fn tls_failed<E>(_: E) -> Error {
    Error::Transport(
        "Mail server TLS negotiation failed. Check the server name and certificate.".into(),
    )
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
            || self.smtp_port == 0
        {
            return Err(Error::Invalid(
                "Configure IMAP and SMTP hosts, ports and credentials".into(),
            ));
        }
        if !self.tls_certificate.is_empty() {
            self.native_certificate()?;
        }
        self.address
            .parse::<lettre::message::Mailbox>()
            .map_err(|_| Error::Invalid("Invalid sender address".into()))?;
        Ok(())
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
        self.validate()?;
        let mut session = self.login()?;
        let result = organize_session(&mut session, ids, seen, flagged, destination, trash, delete);
        let _ = session.logout();
        result
    }

    pub fn folders(&self) -> Result<Vec<colecto_extension_api::Folder>, Error> {
        self.validate()?;
        let mut session = self.login()?;
        let result = folders_session(&mut session);
        let _ = session.logout();
        result
    }

    pub fn query(&self, folder: &str, search: &str) -> Result<Vec<Message>, Error> {
        self.query_with_identity(folder, search)
            .map(|(messages, _)| messages)
    }
    fn native_certificate(&self) -> Result<native_tls::Certificate, Error> {
        if self.tls_certificate.len() > MAX_CERTIFICATE_BYTES
            || self
                .tls_certificate
                .matches("-----BEGIN CERTIFICATE-----")
                .count()
                != 1
        {
            return Err(invalid_certificate());
        }
        native_tls::Certificate::from_pem(self.tls_certificate.as_bytes())
            .map_err(|_| invalid_certificate())
    }
    /// A pinned certificate replaces the system authorities rather than adding to them,
    /// so only that certificate (with a matching host name) is accepted.
    fn tls_connector(&self) -> Result<native_tls::TlsConnector, Error> {
        let mut builder = native_tls::TlsConnector::builder();
        if !self.tls_certificate.is_empty() {
            builder
                .disable_built_in_roots(true)
                .add_root_certificate(self.native_certificate()?);
        }
        builder.build().map_err(remote)
    }
    fn login(&self) -> Result<imap::Session<native_tls::TlsStream<TcpStream>>, Error> {
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
        let tls = self.tls_connector()?;
        let client = if self.imap_starttls {
            let mut plain = imap::Client::new(socket);
            plain.read_greeting().map_err(imap_error)?;
            // `secure` fails unless the server accepts STARTTLS, so a refused or stripped
            // upgrade ends the connection before any credential is sent in plain text.
            plain.secure(&self.imap_host, &tls).map_err(|error| {
                match error {
                imap::error::Error::No(_) | imap::error::Error::Bad(_) => Error::Transport(
                    "Mail server did not accept STARTTLS. Check the IMAP port and security setting."
                        .into(),
                ),
                imap::error::Error::Tls(_) | imap::error::Error::TlsHandshake(_) => {
                    tls_failed(())
                }
                error => imap_error(error),
            }
            })?
        } else {
            let stream = tls.connect(&self.imap_host, socket).map_err(tls_failed)?;
            let mut client = imap::Client::new(stream);
            client.read_greeting().map_err(imap_error)?;
            client
        };
        client
            .login(&self.username, &self.password)
            .map_err(|(error, _)| match error {
                imap::error::Error::No(_) => Error::Transport(
                    "Mail server rejected authentication. Check your username and password.".into(),
                ),
                error => imap_error(error),
            })
    }
    pub(crate) fn query_with_identity(
        &self,
        folder: &str,
        search: &str,
    ) -> Result<(Vec<Message>, String), Error> {
        self.validate()?;
        crate::validate_query(folder, search)?;
        let mut session = self.login()?;
        let result = query_session_with_identity(&mut session, folder, search);
        let _ = session.logout();
        result
    }
    /// IMAP IDLE (RFC 2177) on a read-only Inbox: `Ok(true)` when the server reports a change,
    /// `Ok(false)` when `timeout` passes quietly.
    pub(crate) fn wait_for_inbox(&self, timeout: Duration) -> Result<bool, Error> {
        self.validate()?;
        let mut session = self.login()?;
        let result = idle_session(&mut session, timeout);
        let _ = session.logout();
        result
    }
    /// Fetch one complete message by its Colecto ID without marking it seen.
    pub(crate) fn fetch_raw(&self, id: &str) -> Result<Vec<u8>, Error> {
        self.validate()?;
        let (name, validity, uid) = parse_id(id)?;
        let mut session = self.login()?;
        let result = fetch_session(&mut session, &name, validity, uid);
        let _ = session.logout();
        result
    }

    fn build(
        &self,
        input: &DraftInput,
        attachments: &[Attachment],
    ) -> Result<lettre::Message, Error> {
        crate::validate_draft(input)?;
        let mut builder = lettre::Message::builder()
            .from(
                self.address
                    .parse()
                    .map_err(|_| Error::Invalid("Invalid sender address".into()))?,
            )
            .subject(&input.subject);
        for mailbox in crate::mailboxes(&input.recipient)? {
            builder = builder.to(mailbox);
        }
        for mailbox in crate::mailboxes(&input.cc)? {
            builder = builder.cc(mailbox);
        }
        if !input.in_reply_to.is_empty() {
            builder = builder.in_reply_to(input.in_reply_to.clone());
        }
        if !input.references.is_empty() {
            builder = builder.references(input.references.clone());
        }
        let text = SinglePart::plain(input.body.clone());
        let email = if attachments.is_empty() {
            builder.singlepart(text)
        } else {
            let mut parts = MultiPart::mixed().singlepart(text);
            for file in attachments {
                let content_type = ContentType::parse(&file.content_type)
                    .unwrap_or_else(|_| ContentType::parse("application/octet-stream").unwrap());
                parts = parts.singlepart(
                    lettre::message::Attachment::new(file.name.clone())
                        .body(file.data.clone(), content_type),
                );
            }
            builder.multipart(parts)
        };
        email.map_err(|_| Error::Invalid("Invalid outgoing message".into()))
    }

    pub(crate) fn send(&self, input: &DraftInput, attachments: &[Attachment]) -> Result<(), Error> {
        self.validate()?;
        let email = self.build(input, attachments)?;
        let mut parameters = TlsParameters::builder(self.smtp_host.clone());
        if !self.tls_certificate.is_empty() {
            let certificate = Certificate::from_pem(self.tls_certificate.as_bytes())
                .map_err(|_| invalid_certificate())?;
            parameters = parameters
                .certificate_store(CertificateStore::None)
                .add_root_certificate(certificate);
        }
        let parameters = parameters.build().map_err(remote)?;
        // Port 465 starts with TLS; every other port must upgrade with STARTTLS.
        let tls = if self.smtp_port == 465 {
            Tls::Wrapper(parameters)
        } else {
            Tls::Required(parameters)
        };
        SmtpTransport::builder_dangerous(&self.smtp_host)
            .tls(tls)
            .port(self.smtp_port)
            .timeout(Some(Duration::from_secs(60)))
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

/// IDs are `imap-{validity}-{uid}` for Inbox and `imap-{hex name}-{validity}-{uid}` otherwise.
fn parse_id(id: &str) -> Result<(String, u32, u32), Error> {
    let parts: Vec<_> = id
        .strip_prefix("imap-")
        .ok_or(Error::Missing)?
        .split('-')
        .collect();
    let (name, numbers) = match parts.as_slice() {
        [validity, uid] => ("INBOX".to_owned(), (validity, uid)),
        [hex, validity, uid]
            if hex.len() % 2 == 0 && hex.bytes().all(|b| b.is_ascii_hexdigit()) =>
        {
            let bytes = (0..hex.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&hex[i..i + 2], 16))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|_| Error::Missing)?;
            (
                String::from_utf8(bytes).map_err(|_| Error::Missing)?,
                (validity, uid),
            )
        }
        _ => return Err(Error::Missing),
    };
    let parse = |value: &&str| value.parse::<u32>().map_err(|_| Error::Missing);
    Ok((name, parse(numbers.0)?, parse(numbers.1)?))
}

fn fetch_session<T: std::io::Read + std::io::Write>(
    session: &mut imap::Session<T>,
    name: &str,
    validity: u32,
    uid: u32,
) -> Result<Vec<u8>, Error> {
    let mailbox = session.examine(name).map_err(imap_error)?;
    if mailbox.uid_validity != Some(validity) {
        return Err(Error::Missing);
    }
    let sizes = session
        .uid_fetch(uid.to_string(), "(UID RFC822.SIZE)")
        .map_err(imap_error)?;
    let size = sizes
        .iter()
        .find(|item| item.uid == Some(uid))
        .ok_or(Error::Missing)?
        .size
        .unwrap_or(u32::MAX);
    if size as usize > crate::mime::MAX_RAW {
        return Err(Error::Invalid(
            "This message is larger than 30 MB and cannot be opened here".into(),
        ));
    }
    let fetched = session
        .uid_fetch(uid.to_string(), "(UID FLAGS BODY.PEEK[])")
        .map_err(imap_error)?;
    fetched
        .iter()
        .find_map(|item| item.body())
        .map(<[u8]>::to_vec)
        .ok_or(Error::Missing)
}

fn idle_session<T>(session: &mut imap::Session<T>, timeout: Duration) -> Result<bool, Error>
where
    T: std::io::Read + std::io::Write + imap::extensions::idle::SetReadTimeout,
{
    if !session.capabilities().map_err(imap_error)?.has_str("IDLE") {
        return Err(Error::Invalid(
            "This IMAP server does not support IDLE".into(),
        ));
    }
    session.examine("INBOX").map_err(imap_error)?;
    let outcome = session
        .idle()
        .map_err(imap_error)?
        .wait_with_timeout(timeout)
        .map_err(imap_error)?;
    Ok(outcome == imap::extensions::idle::WaitOutcome::MailboxChanged)
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
            .take(crate::MAX_LIST_MESSAGES)
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
        // Large messages list from headers only; the reader fetches them on demand.
        let large = header.size.unwrap_or(u32::MAX) > 2_000_000;
        let fetched = session
            .uid_fetch(
                uid.to_string(),
                if large {
                    "(UID FLAGS BODY.PEEK[HEADER])"
                } else {
                    "(UID FLAGS BODY.PEEK[])"
                },
            )
            .map_err(imap_error)?;
        for item in fetched.iter() {
            let raw = if large { item.header() } else { item.body() }
                .ok_or_else(|| remote("Missing body"))?;
            let mut message = parse_message(raw, validity, uid)?;
            message.seen = item.flags().contains(&imap::types::Flag::Seen);
            message.flagged = item.flags().contains(&imap::types::Flag::Flagged);
            if large {
                message.body =
                    "This message is larger than 2 MB. Open it to load everything.".into();
            }
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

fn parse_message(raw: &[u8], validity: u32, uid: u32) -> Result<Message, Error> {
    let mail = crate::mime::parse(raw)?;
    let from = mail.headers.get_first_value("From").unwrap_or_default();
    let body = crate::bounded_body(crate::mime::text(&mail)?);
    Ok(Message {
        seen: false,
        flagged: false,
        id: format!("imap-{validity}-{uid}"),
        sender: from.clone(),
        address: from,
        subject: mail.headers.get_first_value("Subject").unwrap_or_default(),
        date: mail.headers.get_first_value("Date").unwrap_or_default(),
        body: if body.is_empty() {
            "This message has no readable text.".into()
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
    fn unicode_imap_ids_are_missing_without_panicking() {
        for id in ["imap-界a-7-42", "imap-éé-7-42", "imap-zz-7-42"] {
            assert!(matches!(parse_id(id), Err(Error::Missing)));
        }
    }
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
                    "UID FETCH 42 (UID FLAGS BODY.PEEK[])",
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
    fn folder_listing_skips_containers_and_reads_status_counts() {
        use colecto_extension_api::FolderRole;
        let server = mock_imap(vec![
            ("LOGIN", String::new()),
            ("LIST", "* LIST (\\Noselect) \"/\" \"[Gmail]\"\r\n* LIST (\\Sent) \"/\" \"Outbox\"\r\n* LIST () \"/\" \"INBOX\"\r\n".into()),
            ("STATUS \"Outbox\" (MESSAGES UNSEEN)", "* STATUS \"Outbox\" (MESSAGES 12 UNSEEN 3)\r\n".into()),
            ("STATUS \"INBOX\" (MESSAGES UNSEEN)", "* STATUS \"INBOX\" (MESSAGES 20 UNSEEN 7)\r\n".into()),
        ]);
        let folders = folders_session(&mut mock_session()).unwrap();
        assert_eq!(folders.len(), 2);
        assert_eq!(folders[0].role, FolderRole::Sent);
        assert_eq!((folders[0].total, folders[0].unread), (12, 3));
        assert_eq!(folders[1].role, FolderRole::Inbox);
        assert_eq!((folders[1].total, folders[1].unread), (20, 7));
        server.join().unwrap();
    }

    #[test]
    fn organizes_with_move_and_copy_fallback_and_checks_validity() {
        for (capabilities, operation) in [
            ("MOVE UIDPLUS", "move"),
            ("UIDPLUS", "copy"),
            ("", "expunge"),
            ("", "stale"),
            ("", "flags"),
            ("", "delete"),
        ] {
            let mut steps = vec![
                ("LOGIN", String::new()),
                (
                    "CAPABILITY",
                    format!(
                        "* CAPABILITY {}\r\n",
                        format!("IMAP4rev1 {capabilities}").trim_end()
                    ),
                ),
                ("SELECT", "* OK [UIDVALIDITY 7] valid\r\n".into()),
            ];
            match operation {
                "move" => steps.push(("UID MOVE 42", String::new())),
                "copy" | "expunge" => {
                    steps.push(("UID COPY 42", String::new()));
                    steps.push(("UID STORE 42 +FLAGS.SILENT (\\Deleted)", String::new()));
                    if operation == "copy" {
                        steps.push(("UID EXPUNGE 42", String::new()));
                    }
                }
                "flags" => {
                    steps.push(("UID STORE 42 +FLAGS.SILENT (\\Seen)", String::new()));
                    steps.push(("UID STORE 42 -FLAGS.SILENT (\\Flagged)", String::new()));
                }
                "delete" => {
                    steps.push(("UID STORE 42 +FLAGS.SILENT (\\Deleted)", String::new()));
                }
                _ => {}
            }
            let server = mock_imap(steps);
            let mut session = mock_session();
            let result = organize_session(
                &mut session,
                &[format!(
                    "imap-{}-42",
                    if operation == "stale" { 8 } else { 7 }
                )],
                (operation == "flags").then_some(true),
                (operation == "flags").then_some(false),
                (!matches!(operation, "flags" | "delete")).then_some("Archive"),
                None,
                operation == "delete",
            );
            assert_eq!(
                result.is_err(),
                operation == "stale",
                "{operation}: {result:?}"
            );
            server.join().unwrap();
        }
    }

    fn mock_imap(steps: Vec<(&'static str, String)>) -> std::thread::JoinHandle<()> {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        MOCK.with(|cell| cell.set(Some(address)));
        std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            write!(socket, "* OK ready\r\n").unwrap();
            for (expected, response) in steps {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let (tag, command) = line.split_once(' ').unwrap();
                assert!(command.starts_with(expected), "unexpected: {command}");
                write!(socket, "{response}{tag} OK complete\r\n").unwrap();
            }
        })
    }
    thread_local!(static MOCK: std::cell::Cell<Option<std::net::SocketAddr>> = const { std::cell::Cell::new(None) });
    fn mock_session() -> imap::Session<TcpStream> {
        let address = MOCK.with(|cell| cell.get()).unwrap();
        let mut client = imap::Client::new(TcpStream::connect(address).unwrap());
        client.read_greeting().unwrap();
        client.login("me", "secret").unwrap()
    }
    /// A scripted server: answers CAPABILITY and EXAMINE, then `idle` decides what IDLE sees.
    fn idle_server(capabilities: &'static str, idle: Option<&'static str>) -> std::net::SocketAddr {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            write!(socket, "* OK ready\r\n").unwrap();
            let mut line = String::new();
            while reader.read_line(&mut line).unwrap_or(0) > 0 {
                let (tag, command) = line
                    .trim_end()
                    .split_once(' ')
                    .unwrap_or((line.trim_end(), ""));
                let tag = tag.to_owned();
                match command {
                    c if c.starts_with("LOGIN") => write!(socket, "{tag} OK in\r\n"),
                    "CAPABILITY" => {
                        write!(socket, "* CAPABILITY {capabilities}\r\n{tag} OK done\r\n")
                    }
                    c if c.starts_with("EXAMINE") => {
                        write!(socket, "* 1 EXISTS\r\n{tag} OK [READ-ONLY]\r\n")
                    }
                    "IDLE" => {
                        write!(socket, "+ idling\r\n").unwrap();
                        if let Some(update) = idle {
                            write!(socket, "{update}\r\n").unwrap();
                        }
                        // The client ends IDLE with DONE; the tagged reply completes it.
                        let mut done = String::new();
                        reader.read_line(&mut done).unwrap();
                        assert_eq!(done.trim_end(), "DONE");
                        write!(socket, "{tag} OK idle done\r\n")
                    }
                    _ => write!(socket, "{tag} OK done\r\n"),
                }
                .unwrap();
                line.clear();
            }
        });
        address
    }
    fn idle(address: std::net::SocketAddr) -> Result<bool, Error> {
        let mut client = imap::Client::new(TcpStream::connect(address).unwrap());
        client.read_greeting().unwrap();
        let mut session = client.login("me", "secret").unwrap();
        idle_session(&mut session, Duration::from_millis(400))
    }
    #[test]
    fn idle_reports_new_mail_quiet_timeouts_and_unsupported_servers() {
        assert!(idle(idle_server("IMAP4rev1 IDLE", Some("* 2 EXISTS"))).unwrap());
        assert!(!idle(idle_server("IMAP4rev1 IDLE", None)).unwrap());
        // Without IDLE the caller keeps polling instead of retrying push.
        assert!(matches!(
            idle(idle_server("IMAP4rev1", None)),
            Err(Error::Invalid(_))
        ));
    }
    #[test]
    fn lists_large_messages_from_headers_and_fetches_them_on_demand() {
        let header = "From: big@example.test\r\nSubject: Big\r\n\r\n";
        let raw = format!("{header}Full body");
        let server = mock_imap(vec![
            ("LOGIN", String::new()),
            (
                "EXAMINE",
                "* 1 EXISTS\r\n* OK [UIDVALIDITY 7] ok\r\n".into(),
            ),
            (
                "FETCH 1:*",
                "* 1 FETCH (UID 5 RFC822.SIZE 3000000)\r\n".into(),
            ),
            (
                "UID FETCH 5 (UID FLAGS BODY.PEEK[HEADER])",
                format!(
                    "* 1 FETCH (UID 5 FLAGS (\\Seen \\Flagged) BODY[HEADER] {{{}}}\r\n{header})\r\n",
                    header.len()
                ),
            ),
            (
                "EXAMINE",
                "* 1 EXISTS\r\n* OK [UIDVALIDITY 8] changed\r\n".into(),
            ),
            (
                "EXAMINE",
                "* 1 EXISTS\r\n* OK [UIDVALIDITY 7] ok\r\n".into(),
            ),
            (
                "UID FETCH 5 (UID RFC822.SIZE)",
                "* 1 FETCH (UID 5 RFC822.SIZE 3000000)\r\n".into(),
            ),
            (
                "UID FETCH 5 (UID FLAGS BODY.PEEK[])",
                format!("* 1 FETCH (UID 5 BODY[] {{{}}}\r\n{raw})\r\n", raw.len()),
            ),
        ]);
        let mut session = mock_session();
        let messages = query_session(&mut session, "inbox", "").unwrap();
        assert_eq!(messages[0].subject, "Big");
        assert!(messages[0].seen && messages[0].flagged);
        assert!(messages[0].body.contains("larger than 2 MB"));
        let (name, validity, uid) = parse_id(&messages[0].id).unwrap();
        assert!(matches!(
            fetch_session(&mut session, &name, validity, uid),
            Err(Error::Missing)
        ));
        assert_eq!(
            fetch_session(&mut session, &name, validity, uid).unwrap(),
            raw.as_bytes()
        );
        server.join().unwrap();
    }
    #[test]
    fn message_ids_round_trip_folder_names() {
        assert_eq!(parse_id("imap-7-42").unwrap(), ("INBOX".into(), 7, 42));
        assert_eq!(
            parse_id("imap-4f7574626f782061726368697665-7-42").unwrap(),
            ("Outbox archive".into(), 7, 42)
        );
        assert!(parse_id("imap-zz-7-42").is_err());
        assert!(parse_id("jmap-7").is_err());
    }
    #[test]
    fn builds_threaded_replies_with_attachments_for_many_recipients() {
        let account = MailAccount {
            imap_host: "localhost".into(),
            imap_port: 993,
            smtp_host: "localhost".into(),
            smtp_port: 587,
            username: "user".into(),
            password: "secret".into(),
            address: "me@example.test".into(),
            imap_starttls: false,
            tls_certificate: String::new(),
        };
        let input = DraftInput {
            recipient: "Ann <ann@example.test>, bob@example.test".into(),
            cc: "cat@example.test".into(),
            subject: "Re: Plans".into(),
            body: "Sounds good".into(),
            in_reply_to: "<m1@example.test>".into(),
            references: "<m0@example.test> <m1@example.test>".into(),
            ..Default::default()
        };
        let file = Attachment {
            name: "notes.txt".into(),
            content_type: "text/plain".into(),
            data: b"hello".to_vec(),
        };
        let email = String::from_utf8(account.build(&input, &[file]).unwrap().formatted()).unwrap();
        for expected in [
            "To: Ann <ann@example.test>, bob@example.test",
            "Cc: cat@example.test",
            "In-Reply-To: <m1@example.test>",
            "References: <m0@example.test> <m1@example.test>",
            "Content-Disposition: attachment; filename=\"notes.txt\"",
        ] {
            assert!(email.contains(expected), "missing {expected} in {email}");
        }
        assert!(
            account
                .build(
                    &DraftInput {
                        recipient: String::new(),
                        ..input
                    },
                    &[]
                )
                .is_err()
        );
    }
    #[test]
    fn rejects_missing_smtp_port_and_invalid_recipient_before_connecting() {
        let mut account = MailAccount {
            imap_host: "localhost".into(),
            imap_port: 993,
            smtp_host: "localhost".into(),
            smtp_port: 0,
            username: "user".into(),
            password: "secret".into(),
            address: "me@example.test".into(),
            imap_starttls: false,
            tls_certificate: String::new(),
        };
        assert!(matches!(account.validate(), Err(Error::Invalid(_))));
        account.smtp_port = 587;
        assert!(matches!(
            account.send(
                &DraftInput {
                    id: None,
                    recipient: "bad\r\nBcc: victim@example.test".into(),
                    subject: "Hi".into(),
                    body: "Hello".into(),
                    ..Default::default()
                },
                &[]
            ),
            Err(Error::Invalid(_))
        ));
    }

    const PINNED_CERT: &str = include_str!("../tests/fixtures/pinned-cert.pem");
    const PINNED_KEY: &str = include_str!("../tests/fixtures/pinned-key.pem");
    const OTHER_CERT: &str = include_str!("../tests/fixtures/other-cert.pem");
    const OTHER_KEY: &str = include_str!("../tests/fixtures/other-key.pem");

    fn acceptor(cert: &str, key: &str) -> native_tls::TlsAcceptor {
        let identity = native_tls::Identity::from_pkcs8(cert.as_bytes(), key.as_bytes()).unwrap();
        native_tls::TlsAcceptor::new(identity).unwrap()
    }
    fn pinned_account(host: &str, imap_port: u16, smtp_port: u16, starttls: bool) -> MailAccount {
        MailAccount {
            imap_host: host.into(),
            imap_port,
            smtp_host: host.into(),
            smtp_port,
            username: "user".into(),
            password: "secret".into(),
            address: "me@example.test".into(),
            imap_starttls: starttls,
            tls_certificate: PINNED_CERT.into(),
        }
    }
    fn read_line(reader: &mut impl std::io::BufRead) -> String {
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        line
    }
    /// A plain IMAP server that answers STARTTLS with `reply`, then (on OK) switches to TLS
    /// with `cert` and accepts LOGIN. Returns the lines it read after the STARTTLS answer.
    fn starttls_imap(
        reply: &'static str,
        cert: &'static str,
        key: &'static str,
    ) -> (u16, std::thread::JoinHandle<Vec<String>>) {
        use std::io::{BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            write!(socket, "* OK ready\r\n").unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            let line = read_line(&mut reader);
            let (tag, command) = line.split_once(' ').unwrap();
            assert_eq!(command.trim_end(), "STARTTLS");
            write!(socket, "{tag} {reply}\r\n").unwrap();
            if !reply.starts_with("OK") {
                return vec![read_line(&mut reader)];
            }
            let Ok(stream) = acceptor(cert, key).accept(socket) else {
                return Vec::new();
            };
            let mut reader = BufReader::new(stream);
            let line = read_line(&mut reader);
            let tag = line.split_once(' ').unwrap().0.to_owned();
            write!(reader.get_mut(), "{tag} OK logged in\r\n").unwrap();
            vec![line]
        });
        (port, handle)
    }

    #[test]
    fn imap_starttls_upgrades_with_a_pinned_certificate_before_login() {
        let (port, server) = starttls_imap("OK begin TLS", PINNED_CERT, PINNED_KEY);
        let account = pinned_account("127.0.0.1", port, 1025, true);
        account.login().unwrap();
        let lines = server.join().unwrap();
        assert!(lines[0].contains("LOGIN"), "{lines:?}");
    }
    #[test]
    fn imap_never_logs_in_when_starttls_is_refused_or_the_certificate_differs() {
        let (port, server) = starttls_imap("NO not here", PINNED_CERT, PINNED_KEY);
        let error = pinned_account("127.0.0.1", port, 1025, true)
            .login()
            .err()
            .unwrap();
        assert!(error.to_string().contains("STARTTLS"), "{error}");
        // The client closes the connection instead of sending credentials in plain text.
        assert_eq!(server.join().unwrap(), vec![String::new()]);

        let (port, server) = starttls_imap("OK begin TLS", OTHER_CERT, OTHER_KEY);
        let error = pinned_account("127.0.0.1", port, 1025, true)
            .login()
            .err()
            .unwrap();
        assert!(
            error.to_string().contains("TLS negotiation failed"),
            "{error}"
        );
        assert!(server.join().unwrap().is_empty());
    }
    #[test]
    fn implicit_tls_checks_the_pinned_certificate_and_host_name() {
        for (host, cert, key, accepted) in [
            ("127.0.0.1", PINNED_CERT, PINNED_KEY, true),
            ("127.0.0.1", OTHER_CERT, OTHER_KEY, false),
            // The pinned certificate names 127.0.0.1, not localhost.
            ("localhost", PINNED_CERT, PINNED_KEY, false),
        ] {
            use std::io::{BufReader, Write};
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let port = listener.local_addr().unwrap().port();
            let server = std::thread::spawn(move || {
                let (socket, _) = listener.accept().unwrap();
                let Ok(mut stream) = acceptor(cert, key).accept(socket) else {
                    return false;
                };
                write!(stream, "* OK ready\r\n").unwrap();
                let mut reader = BufReader::new(stream);
                let line = read_line(&mut reader);
                let tag = line.split_once(' ').unwrap().0.to_owned();
                write!(reader.get_mut(), "{tag} OK logged in\r\n").unwrap();
                true
            });
            let result = pinned_account(host, port, 465, false).login();
            assert_eq!(result.is_ok(), accepted, "{host}");
            assert_eq!(server.join().unwrap(), accepted, "{host}");
        }
    }
    #[test]
    fn rejects_malformed_or_multiple_pinned_certificates() {
        let mut account = pinned_account("127.0.0.1", 993, 587, false);
        assert!(account.validate().is_ok());
        for pem in [
            "not a certificate".to_owned(),
            format!("{PINNED_CERT}{OTHER_CERT}"),
        ] {
            account.tls_certificate = pem;
            assert!(matches!(account.validate(), Err(Error::Invalid(_))));
        }
    }
    /// An SMTP server on a custom port. It offers STARTTLS when `starttls` is set and then
    /// accepts one message; it returns every command it read.
    fn smtp_server(starttls: bool) -> (u16, std::thread::JoinHandle<Vec<String>>) {
        use std::io::{BufRead, BufReader, Write};
        fn exchange<S: std::io::Read + std::io::Write>(
            reader: &mut BufReader<S>,
            seen: &mut Vec<String>,
            reply: &str,
        ) -> String {
            let line = read_line(reader);
            seen.push(line.clone());
            write!(reader.get_mut(), "{reply}\r\n").unwrap();
            line
        }
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            write!(socket, "220 test ESMTP\r\n").unwrap();
            let mut seen = Vec::new();
            let mut plain = BufReader::new(socket.try_clone().unwrap());
            let offer = if starttls {
                "250-test\r\n250 STARTTLS"
            } else {
                "250-test\r\n250 AUTH PLAIN"
            };
            exchange(&mut plain, &mut seen, offer);
            if !starttls {
                // Record whatever the client does next; it must not authenticate.
                seen.push(read_line(&mut plain));
                return seen;
            }
            exchange(&mut plain, &mut seen, "220 go ahead");
            let mut tls = BufReader::new(acceptor(PINNED_CERT, PINNED_KEY).accept(socket).unwrap());
            exchange(&mut tls, &mut seen, "250-test\r\n250 AUTH PLAIN");
            exchange(&mut tls, &mut seen, "235 ok");
            exchange(&mut tls, &mut seen, "250 ok");
            exchange(&mut tls, &mut seen, "250 ok");
            exchange(&mut tls, &mut seen, "354 go");
            loop {
                let line = read_line(&mut tls);
                if line == ".\r\n" || line.is_empty() {
                    break;
                }
            }
            write!(tls.get_mut(), "250 queued\r\n").unwrap();
            let mut quit = String::new();
            let _ = tls.read_line(&mut quit);
            let _ = write!(tls.get_mut(), "221 bye\r\n");
            seen
        });
        (port, handle)
    }
    #[test]
    fn smtp_uses_required_starttls_on_custom_ports_with_a_pinned_certificate() {
        let draft = DraftInput {
            recipient: "ann@example.test".into(),
            subject: "Hi".into(),
            body: "Hello".into(),
            ..Default::default()
        };
        let (port, server) = smtp_server(true);
        pinned_account("127.0.0.1", 993, port, false)
            .send(&draft, &[])
            .unwrap();
        let seen = server.join().unwrap();
        assert_eq!(seen[1].trim_end(), "STARTTLS");
        assert!(seen[3].starts_with("AUTH PLAIN"), "{seen:?}");

        let (port, server) = smtp_server(false);
        assert!(
            pinned_account("127.0.0.1", 993, port, false)
                .send(&draft, &[])
                .is_err()
        );
        let seen = server.join().unwrap();
        assert!(
            seen.iter().all(|line| !line.starts_with("AUTH")),
            "{seen:?}"
        );
    }
}

#[cfg(test)]
mod folder_tests {
    use super::*;
    use colecto_extension_api::FolderRole;
    #[test]
    fn roles_prefer_special_use_and_recognize_provider_names() {
        for (name, attributes, expected) in [
            ("Sent", vec![], FolderRole::Sent),
            ("[Gmail]/Deleted Messages", vec![], FolderRole::Trash),
            ("Spam", vec![], FolderRole::Junk),
            ("INBOX", vec![], FolderRole::Inbox),
            ("Projects/2026", vec![], FolderRole::Other),
            ("Sent", vec!["\\Archive"], FolderRole::Archive),
            ("Everything", vec!["\\All"], FolderRole::Archive),
            ("Drafts", vec![], FolderRole::Drafts),
        ] {
            assert_eq!(folder_role(name, &attributes), expected);
        }
    }
}

fn folder_role(name: &str, attributes: &[&str]) -> colecto_extension_api::FolderRole {
    use colecto_extension_api::FolderRole::*;
    for (attribute, role) in [
        ("\\Inbox", Inbox),
        ("\\Sent", Sent),
        ("\\Drafts", Drafts),
        ("\\Archive", Archive),
        ("\\Trash", Trash),
        ("\\Junk", Junk),
        ("\\All", Archive),
    ] {
        if attributes
            .iter()
            .any(|value| value.eq_ignore_ascii_case(attribute))
        {
            return role;
        }
    }
    match name
        .trim_start_matches("[Gmail]/")
        .to_ascii_lowercase()
        .as_str()
    {
        "inbox" => Inbox,
        "sent" | "sent items" | "sent messages" | "sent mail" => Sent,
        "drafts" => Drafts,
        "archive" | "all mail" => Archive,
        "trash" | "deleted items" | "deleted messages" => Trash,
        "junk" | "spam" => Junk,
        _ => Other,
    }
}

fn folders_session<T: std::io::Read + std::io::Write>(
    session: &mut imap::Session<T>,
) -> Result<Vec<colecto_extension_api::Folder>, Error> {
    use imap::types::NameAttribute;
    let names = session.list(Some(""), Some("*")).map_err(imap_error)?;
    let mut folders = Vec::new();
    for name in names.iter() {
        if name.attributes().contains(&NameAttribute::NoSelect) {
            continue;
        }
        let attributes: Vec<&str> = name
            .attributes()
            .iter()
            .filter_map(|a| match a {
                NameAttribute::Custom(value) => Some(value.as_ref()),
                _ => None,
            })
            .collect();
        session
            .status(name.name(), "(MESSAGES UNSEEN)")
            .map_err(imap_error)?;
        // imap 2.x delivers STATUS data through its unsolicited response channel.
        let mut total = None;
        let mut unread = None;
        while let Ok(response) = session.unsolicited_responses.try_recv() {
            if let imap::types::UnsolicitedResponse::Status {
                mailbox,
                attributes,
            } = response
                && mailbox == name.name()
            {
                for attribute in attributes {
                    match attribute {
                        imap::types::StatusAttribute::Messages(value) => total = Some(value),
                        imap::types::StatusAttribute::Unseen(value) => unread = Some(value),
                        _ => {}
                    }
                }
            }
        }
        folders.push(colecto_extension_api::Folder {
            id: name.name().into(),
            name: name.name().into(),
            role: folder_role(name.name(), &attributes),
            unread: unread.ok_or_else(|| remote("Missing UNSEEN"))?,
            total: total.ok_or_else(|| remote("Missing MESSAGES"))?,
        });
    }
    Ok(folders)
}

fn organize_session<T: std::io::Read + std::io::Write>(
    session: &mut imap::Session<T>,
    ids: &[String],
    seen: Option<bool>,
    flagged: Option<bool>,
    destination: Option<&str>,
    trash: Option<&str>,
    delete: bool,
) -> Result<(), Error> {
    let parsed = ids
        .iter()
        .map(|id| parse_id(id))
        .collect::<Result<Vec<_>, _>>()?;
    let capabilities = session.capabilities().map_err(imap_error)?;
    let can_move = capabilities.has_str("MOVE");
    let uidplus = capabilities.has_str("UIDPLUS");
    for (name, validity, uid) in parsed {
        if session.select(&name).map_err(imap_error)?.uid_validity != Some(validity) {
            return Err(Error::Invalid("Mailbox UIDVALIDITY changed".into()));
        }
        let uid = uid.to_string();
        for (value, flag) in [(seen, "\\Seen"), (flagged, "\\Flagged")] {
            if let Some(value) = value {
                session
                    .uid_store(
                        &uid,
                        format!("{}FLAGS.SILENT ({flag})", if value { "+" } else { "-" }),
                    )
                    .map_err(imap_error)?;
            }
        }
        let target = if delete {
            trash.filter(|target| *target != name)
        } else {
            destination
        };
        if let Some(target) = target {
            if target == name {
                continue;
            }
            if can_move {
                session.uid_mv(&uid, target).map_err(imap_error)?;
                continue;
            }
            session.uid_copy(&uid, target).map_err(imap_error)?;
        }
        if target.is_some() || delete {
            session
                .uid_store(&uid, "+FLAGS.SILENT (\\Deleted)")
                .map_err(imap_error)?;
            // A plain EXPUNGE would also remove messages other clients only marked as
            // deleted, so without UIDPLUS the message stays flagged until the server or
            // another client expunges it.
            if uidplus {
                session.uid_expunge(&uid).map_err(imap_error)?;
            }
        }
    }
    Ok(())
}
