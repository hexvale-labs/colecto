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
