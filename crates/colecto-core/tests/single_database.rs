use colecto_core::MailStore;
use colecto_extension_api::DraftInput;

#[test]
fn canonical_drafts_and_labels_ignore_stale_legacy_files() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = MailStore::open(dir.path()).unwrap();
    let draft = store
        .save_draft(DraftInput {
            id: None,
            recipient: "a@example.com".into(),
            subject: "canonical".into(),
            body: "body".into(),
            ..Default::default()
        })
        .unwrap();
    store.update_label("Work", None, false).unwrap();
    drop(store);
    assert!(dir.path().join("colecto.db").exists());
    std::fs::write(dir.path().join("drafts.json"), b"stale invalid draft").unwrap();
    std::fs::write(dir.path().join("labels.json"), b"stale invalid label").unwrap();
    let store = MailStore::open(dir.path()).unwrap();
    assert_eq!(store.drafts()[0].id, draft.id);
    assert!(store.labels().unwrap().contains_key("Work"));
}

#[test]
fn fresh_store_ignores_unsupported_legacy_documents() {
    let dir = tempfile::tempdir().unwrap();
    for name in [
        "drafts.json",
        "labels.json",
        "mail-cache.db",
        "mail-cache.json",
    ] {
        std::fs::write(dir.path().join(name), b"unsupported legacy content").unwrap();
    }
    let store = MailStore::open(dir.path()).unwrap();
    assert!(store.drafts().is_empty());
    assert!(store.labels().unwrap().is_empty());
}

#[test]
fn invalid_canonical_drafts_are_preserved_and_rejected() {
    for content in [
        "invalid",
        r#"{"version":99,"drafts":[]}"#,
        r#"{"version":1,"drafts":[{"id":"draft-1","recipient":"","subject":"","body":""},{"id":"draft-1","recipient":"","subject":"","body":""}]}"#,
    ] {
        let dir = tempfile::tempdir().unwrap();
        let store = MailStore::open(dir.path()).unwrap();
        drop(store);
        let database = colecto_core::Database::open(dir.path()).unwrap();
        database
            .set_record("account:demo", "drafts", 1, content.as_bytes())
            .unwrap();
        assert!(MailStore::open(dir.path()).is_err());
        assert_eq!(
            database
                .get_record("account:demo", "drafts", 1)
                .unwrap()
                .unwrap(),
            content.as_bytes()
        );
    }
}
