use colecto_core::{Database, DraftInput, MailStore};
use std::path::Path;

fn sample_data(dir: &Path) -> String {
    let mut store = MailStore::open(dir).unwrap();
    let draft = store
        .save_draft(DraftInput {
            id: None,
            recipient: "a@example.com".into(),
            subject: "kept".into(),
            body: "body".into(),
            ..Default::default()
        })
        .unwrap();
    store.update_label("Work", None, false).unwrap();
    store.update_label("Work", Some("welcome"), false).unwrap();
    drop(store);
    let database = Database::open(dir).unwrap();
    database
        .write_settings(
            "core",
            &[("list_density".into(), Some("\"compact\"".into()))],
        )
        .unwrap();
    database
        .write(vec![
            (
                "INSERT INTO api_keys(id,name,accounts,capabilities,created_at,verifier) VALUES (?1,'Assistant','[\"demo\"]','[\"messages.list\"]',100,?2)",
                vec![KEY_ID.into(), VERIFIER.to_vec().into()],
            ),
            (
                "INSERT INTO send_approvals(id,requester,account,sender,snapshot,created_at,expires_at,status) VALUES ('request','key','demo','me@example.com',NULL,100,700,'pending')",
                vec![],
            ),
            (
                "INSERT INTO ai_summaries(account,message_id,summary,created_at) VALUES ('demo','welcome','Brief summary',100)",
                vec![],
            ),
            (
                "INSERT INTO ai_autolabel_seen(account,message_id,seen_at) VALUES ('demo','welcome',100)",
                vec![],
            ),
        ])
        .unwrap();
    draft.id
}

const KEY_ID: &str = "abababababababababababababababababababababababababababababababab";
const VERIFIER: [u8; 32] = [
    0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
    26, 27, 28, 29, 30, 31,
];

#[test]
fn drafts_and_labels_live_in_the_database() {
    let dir = tempfile::tempdir().unwrap();
    let id = sample_data(dir.path());
    let store = MailStore::open(dir.path()).unwrap();
    assert_eq!(store.drafts()[0].id, id);
    assert!(store.labels().unwrap().contains_key("Work"));
}

#[test]
fn invalid_drafts_are_kept_and_rejected() {
    let dir = tempfile::tempdir().unwrap();
    drop(MailStore::open(dir.path()).unwrap());
    let database = Database::open(dir.path()).unwrap();
    database
        .write(vec![(
            "INSERT INTO drafts(account,id,recipient,cc,subject,body,in_reply_to,message_references) VALUES ('demo',1,'','','two\nlines','','','')",
            vec![],
        )])
        .unwrap();
    assert!(MailStore::open(dir.path()).is_err());
    assert_eq!(
        database
            .query("SELECT subject FROM drafts", vec![])
            .unwrap()[0]
            .text(0)
            .unwrap(),
        "two\nlines"
    );
}

#[test]
fn all_entities_reopen_cleanly_in_single_database() {
    let dir = tempfile::tempdir().unwrap();
    sample_data(dir.path());
    let store = MailStore::open(dir.path()).unwrap();
    assert_eq!(store.drafts()[0].subject, "kept");
    assert_eq!(store.labels().unwrap()["Work"], ["welcome"]);
    drop(store);

    let database = Database::open(dir.path()).unwrap();
    assert_eq!(
        database.settings("core").unwrap(),
        vec![("list_density".to_string(), "\"compact\"".to_string())]
    );
    let key = &database
        .query("SELECT id,name,accounts,verifier FROM api_keys", vec![])
        .unwrap()[0];
    assert_eq!(key.text(0).unwrap(), KEY_ID);
    assert_eq!(key.text(1).unwrap(), "Assistant");
    assert_eq!(key.text(2).unwrap(), r#"["demo"]"#);
    assert_eq!(key.blob(3).unwrap(), VERIFIER);

    let request = &database
        .query("SELECT sender,status FROM send_approvals", vec![])
        .unwrap()[0];
    assert_eq!(request.text(0).unwrap(), "me@example.com");
    assert_eq!(request.text(1).unwrap(), "pending");

    let summary = &database
        .query(
            "SELECT summary FROM ai_summaries WHERE account='demo' AND message_id='welcome'",
            vec![],
        )
        .unwrap()[0];
    assert_eq!(summary.text(0).unwrap(), "Brief summary");

    let seen = &database
        .query(
            "SELECT seen_at FROM ai_autolabel_seen WHERE account='demo' AND message_id='welcome'",
            vec![],
        )
        .unwrap()[0];
    assert_eq!(seen.integer(0).unwrap(), 100);
}

/// Best effort: Turso writes the SQLite file format, so the stock `sqlite3`
/// tool can read Colecto's database. Skips where `sqlite3` is not installed.
#[test]
fn sqlite_can_read_the_database() {
    let dir = tempfile::tempdir().unwrap();
    sample_data(dir.path());
    Database::open(dir.path()).unwrap().checkpoint().unwrap();
    let output = match std::process::Command::new("sqlite3")
        .arg(dir.path().join("colecto.db"))
        .arg("PRAGMA integrity_check; SELECT value FROM settings WHERE key='list_density';")
        .output()
    {
        Ok(output) => output,
        Err(_) => return eprintln!("sqlite3 not installed; skipping"),
    };
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "ok\n\"compact\"\n");
}
