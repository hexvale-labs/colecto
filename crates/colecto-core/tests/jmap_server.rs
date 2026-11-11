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

#[test]
fn organizes_against_scripted_jmap() {
    use serde_json::{Value, json};
    use std::io::{BufRead, BufReader, Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let base = url.clone();
    let server = std::thread::spawn(move || {
        let mut sets = 0;
        let mut reads = 0;
        let mut folders = 0;
        while sets < 5 || folders < 5 {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = BufReader::new(socket.try_clone().unwrap());
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                    length = value.trim().parse().unwrap();
                }
            }
            let response = if first.starts_with("GET") {
                json!({"apiUrl":base,"primaryAccounts":{"urn:ietf:params:jmap:mail":"a"},"accounts":{"a":{"accountCapabilities":{"urn:ietf:params:jmap:mail":{}}}}})
            } else {
                let mut bytes = vec![0;length]; reader.read_exact(&mut bytes).unwrap();
                let request: Value = serde_json::from_slice(&bytes).unwrap();
                let call = &request["methodCalls"][0];
                let result = match call[0].as_str().unwrap() {
                    "Mailbox/get" => {
                        folders += 1;
                        let list = if folders == 5 { json!([]) } else { json!([
                            {"id":"archive","name":"Archive","role":"archive","totalEmails":0,"unreadEmails":0},
                            {"id":"trash","name":"Trash","role":"trash","totalEmails":0,"unreadEmails":0}
                        ]) };
                        json!({"list":list})
                    }
                    "Email/get" => {
                        reads += 1;
                        json!({"list":[{"id":"x","mailboxIds": if reads == 1 {json!({"inbox":true})} else {json!({"trash":true})}}]})
                    }
                    "Email/set" => {
                        sets += 1;
                        let args = &call[1];
                        match sets {
                            1 => assert_eq!(args["update"]["x"], json!({"keywords/$seen":true,"keywords/$flagged":null})),
                            2 | 3 => assert_eq!(args["update"]["x"]["mailboxIds"], json!({"archive":true})),
                            4 => assert_eq!(args["update"]["x"]["mailboxIds"], json!({"trash":true})),
                            5 => assert_eq!(args["destroy"], json!(["x"])),
                            _ => unreachable!()
                        }
                        json!({"updated":{"x":null},"destroyed":["x"]})
                    }
                    _ => panic!("unexpected call: {call}")
                };
                json!({"methodResponses":[[call[0],result,call[2]]]})
            }.to_string();
            write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response}",response.len()).unwrap();
        }
    });
    let dir = tempfile::tempdir().unwrap();
    let mut store = MailStore::open(dir.path()).unwrap();
    store
        .connect_jmap(JmapAccount {
            session_url: url,
            token: "secret".into(),
            username: String::new(),
            address: "me@example.test".into(),
        })
        .unwrap();
    let ids = vec!["jmap-x".into()];
    store.set_flags(&ids, Some(true), Some(false)).unwrap();
    store.move_to(&ids, "archive").unwrap();
    store.archive(&ids).unwrap();
    store.delete(&ids).unwrap();
    store.delete(&ids).unwrap();
    assert_eq!(
        store.archive(&ids).unwrap_err().to_string(),
        "This account has no Archive folder"
    );
    server.join().unwrap();
}
