//! End-to-end checks against a real IMAP and SMTP server when `COLECTO_MAIL_TEST_CERT` is set;
//! otherwise these tests skip.
use colecto_core::{DraftInput, MailAccount, MailStore, OutgoingAttachment};
use std::time::{Duration, Instant};

const IMAP_PORT: u16 = 3993;
// Colecto starts SMTP with TLS on port 465 and requires STARTTLS on every other port.
const SMTP_PORT: u16 = 465;

fn certificate() -> Option<String> {
    let path = std::env::var_os("COLECTO_MAIL_TEST_CERT")?;
    Some(std::fs::read_to_string(path).unwrap())
}

fn settings(user: &str, certificate: &str) -> MailAccount {
    MailAccount {
        imap_host: "localhost".into(),
        imap_port: IMAP_PORT,
        smtp_host: "localhost".into(),
        smtp_port: SMTP_PORT,
        username: format!("{user}@example.com"),
        password: "password".into(),
        address: format!("{user}@example.com"),
        imap_starttls: false,
        tls_certificate: certificate.into(),
    }
}

fn account(user: &str, certificate: &str) -> (tempfile::TempDir, MailStore) {
    let directory = tempfile::tempdir().unwrap();
    let mut store = MailStore::open(directory.path()).unwrap();
    store.connect_account(settings(user, certificate)).unwrap();
    (directory, store)
}

fn create_folder(user: &str, certificate: &str, name: &str) {
    let tls = native_tls::TlsConnector::builder()
        .disable_built_in_roots(true)
        .add_root_certificate(native_tls::Certificate::from_pem(certificate.as_bytes()).unwrap())
        .build()
        .unwrap();
    let client = imap::connect(("localhost", IMAP_PORT), "localhost", &tls).unwrap();
    let mut session = client
        .login(format!("{user}@example.com"), "password")
        .map_err(|(error, _)| error)
        .unwrap();
    session.create(name).unwrap();
    session.logout().unwrap();
}

fn message(subject: &str) -> DraftInput {
    DraftInput {
        id: None,
        recipient: "Second User <user2@example.com>".into(),
        cc: String::new(),
        subject: subject.into(),
        body: "Sent by the Colecto IMAP and SMTP end-to-end test.".into(),
        in_reply_to: String::new(),
        references: String::new(),
        forward_of: None,
        attachments: Vec::new(),
    }
}

/// Poll a folder until a message with this subject appears.
fn wait_for(store: &mut MailStore, folder: &str, subject: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let found = store.query(folder, "").unwrap();
        if let Some(message) = found.iter().find(|m| m.subject == subject) {
            return message.id.clone();
        }
        assert!(
            Instant::now() < deadline,
            "{subject} never reached {folder}"
        );
        std::thread::sleep(Duration::from_millis(500));
    }
}

fn unique(label: &str) -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{label} {nanos}")
}

#[test]
fn sends_with_attachments_receives_searches_and_files_in_sent() {
    let Some(certificate) = certificate() else {
        return;
    };
    create_folder("user1", &certificate, "INBOX.Sent");
    let (_one, mut sender) = account("user1", &certificate);
    let (_two, mut recipient) = account("user2", &certificate);
    let subject = unique("Quarterly report");
    let mut draft = message(&subject);
    draft.attachments.push(OutgoingAttachment {
        name: "report.txt".into(),
        content_type: "text/plain".into(),
        data: "UmVwb3J0IGJvZHk=".into(), // "Report body"
    });
    sender.send(draft).unwrap();

    let id = wait_for(&mut recipient, "inbox", &subject);
    let content = recipient.content(&id).unwrap();
    assert!(content.text.contains("IMAP and SMTP end-to-end test"));
    assert_eq!(content.attachments.len(), 1);
    assert_eq!(content.attachments[0].name, "report.txt");
    let (_, bytes) = recipient.attachment(&id, 0).unwrap();
    assert_eq!(bytes, b"Report body");

    let found = recipient.query("inbox", &subject).unwrap();
    assert!(found.iter().any(|m| m.id == id), "search did not find it");
    let filtered = recipient
        .query(
            "inbox",
            &format!("{subject} from:user1@example.com is:unread"),
        )
        .unwrap();
    assert!(
        filtered.iter().any(|m| m.id == id),
        "filters did not find it"
    );
    assert!(
        recipient
            .query("inbox", &format!("{subject} from:nobody@example.com"))
            .unwrap()
            .is_empty()
    );
    assert!(
        recipient
            .query("inbox", &format!("{subject} is:starred"))
            .unwrap()
            .is_empty()
    );
    assert!(
        recipient
            .query("inbox", "no such words")
            .unwrap()
            .is_empty()
    );

    wait_for(&mut sender, "sent", &subject);
}

#[test]
fn rejects_a_certificate_it_was_not_told_to_trust() {
    let Some(_) = certificate() else {
        return;
    };
    let directory = tempfile::tempdir().unwrap();
    let mut store = MailStore::open(directory.path()).unwrap();
    store.connect_account(settings("user1", "")).unwrap();
    // The server's certificate is self-signed, so without pinning it TLS must fail.
    assert!(store.query("inbox", "").is_err());
}
