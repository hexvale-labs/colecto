use colecto_core::{Database, MailStore};
use colecto_extension_api::DraftInput;
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

/// Every database a released schema version wrote must still open and keep its data.
/// `tests/fixtures/schema/v<N>/colecto.db` holds one per version; see docs/storage.md.
#[test]
fn every_schema_fixture_opens_and_keeps_its_data() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/schema");
    let mut versions = 0;
    for entry in std::fs::read_dir(&root).unwrap() {
        let fixture = entry.unwrap().path().join("colecto.db");
        let dir = tempfile::tempdir().unwrap();
        std::fs::copy(&fixture, dir.path().join("colecto.db")).unwrap();
        let store = MailStore::open(dir.path()).unwrap();
        assert_eq!(store.drafts()[0].subject, "kept", "{}", fixture.display());
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
        assert!(
            database
                .query("SELECT name FROM records WHERE name IN ('drafts','labels','api-keys','send-approvals')", vec![])
                .unwrap()
                .is_empty()
        );
        versions += 1;
    }
    assert_eq!(
        versions,
        colecto_core::SCHEMA_VERSION,
        "one fixture per schema version"
    );
}

/// Writes the fixture for the current schema version:
/// `cargo test -p colecto-core --test single_database -- --ignored write_schema_fixture`.
#[test]
#[ignore]
fn write_schema_fixture() {
    let dir = tempfile::tempdir().unwrap();
    sample_data(dir.path());
    Database::open(dir.path()).unwrap().checkpoint().unwrap();
    let target = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
        "tests/fixtures/schema/v{}",
        colecto_core::SCHEMA_VERSION
    ));
    std::fs::create_dir_all(&target).unwrap();
    std::fs::copy(dir.path().join("colecto.db"), target.join("colecto.db")).unwrap();
}

/// Best effort, not a promise: Turso writes the SQLite file format, so the stock `sqlite3`
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
