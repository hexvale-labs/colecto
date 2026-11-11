//! End-to-end checks against a real JMAP server. Run with `just test jmap`, which starts the
//! Cyrus test server in Docker and sets `COLECTO_JMAP_TEST_URL`; otherwise these tests skip.
//! Push is covered by unit tests only: that server never emits StateChange events.
use colecto_core::{JmapAccount, MailStore};
use colecto_extension_api::{DraftInput, OutgoingAttachment};
use std::time::{Duration, Instant};

fn account(user: &str) -> Option<(tempfile::TempDir, MailStore)> {
    let url = std::env::var("COLECTO_JMAP_TEST_URL").ok()?;
    let directory = tempfile::tempdir().unwrap();
    let mut store = MailStore::open(directory.path()).unwrap();
    store
        .connect_jmap(JmapAccount {
            session_url: url,
            token: "password".into(),
            address: format!("{user}@example.com"),
            username: user.into(),
        })
        .unwrap();
    Some((directory, store))
}

fn message(subject: &str) -> DraftInput {
    DraftInput {
        id: None,
        recipient: "Second User <user2@example.com>".into(),
        cc: String::new(),
        subject: subject.into(),
        body: "Sent by the Colecto JMAP end-to-end test.".into(),
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
    let Some((_one, mut sender)) = account("user1") else {
        return;
    };
    let (_two, mut recipient) = account("user2").unwrap();
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
    assert!(content.text.contains("Colecto JMAP end-to-end test"));
    assert_eq!(content.attachments.len(), 1);
    assert_eq!(content.attachments[0].name, "report.txt");
    let (_, bytes) = recipient.attachment(&id, 0).unwrap();
    assert_eq!(bytes, b"Report body");

    let found = recipient.query("inbox", &subject).unwrap();
    assert!(found.iter().any(|m| m.id == id), "search did not find it");

    wait_for(&mut sender, "sent", &subject);
}
