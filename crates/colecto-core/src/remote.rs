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
    // Protocol responses may contain credentials or message content.
    Error::Transport("Mail server operation failed. Check server settings and credentials. Sending failures may have an uncertain outcome; check Sent before retrying.".into())
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

    pub fn receive(&self) -> Result<Vec<Message>, Error> {
        self.validate()?;
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
        let stream = tls.connect(&self.imap_host, socket).map_err(remote)?;
        let mut client = imap::Client::new(stream);
        client.read_greeting().map_err(remote)?;
        let mut session = client
            .login(&self.username, &self.password)
            .map_err(|(error, _)| remote(error))?;
        let result = (|| {
            let mailbox = session.examine("INBOX").map_err(remote)?;
            let validity = mailbox
                .uid_validity
                .ok_or_else(|| remote("Missing UIDVALIDITY"))?;
            if mailbox.exists == 0 {
                return Ok(Vec::new());
            }
            // shortcut: latest 100 INBOX messages only; add pagination before full mailbox sync.
            let range = format!("{}:*", mailbox.exists.saturating_sub(99).max(1));
            let headers = session.fetch(&range, "UID RFC822.SIZE").map_err(remote)?;
            let mut messages = Vec::new();
            for header in headers.iter().rev() {
                let uid = header.uid.ok_or_else(|| remote("Missing UID"))?;
                if header.size.unwrap_or(u32::MAX) > 2_000_000 {
                    continue;
                }
                let fetched = session
                    .uid_fetch(uid.to_string(), "UID BODY.PEEK[]")
                    .map_err(remote)?;
                for item in fetched.iter() {
                    let raw = item.body().ok_or_else(|| remote("Missing body"))?;
                    messages.push(parse_message(raw, validity, uid)?);
                }
            }
            Ok(messages)
        })();
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
            .map_err(remote)?;
        Ok(())
    }
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
    let body = text_body(&mail)?;
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

#[cfg(test)]
mod tests {
    use super::*;
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
