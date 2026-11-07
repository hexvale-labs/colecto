use colecto_core::MailStore;
use colecto_extension_api::DraftInput;
use std::fs;

fn input(id: Option<String>, subject: &str) -> DraftInput {
    DraftInput {
        id,
        recipient: "friend@example.test".into(),
        subject: subject.into(),
        body: "Hello 世界".into(),
    }
}

#[test]
fn sample_mail_is_readable_and_unknown_ids_are_missing() {
    let dir = tempfile::tempdir().unwrap();
    let store = MailStore::open(dir.path()).unwrap();
    assert!(!store.messages().is_empty());
    let message = store.message("welcome").unwrap();
    assert_eq!(message.subject, "A quieter place for your mail");
    assert!(message.body.contains("Colecto"));
    assert!(store.message("missing").is_none());
}

#[test]
fn drafts_survive_restart_and_updates_keep_their_id() {
    let dir = tempfile::tempdir().unwrap();
    let id;
    {
        let mut store = MailStore::open(dir.path()).unwrap();
        let draft = store.save_draft(input(None, "First")).unwrap();
        id = draft.id;
        let updated = store
            .save_draft(input(Some(id.clone()), "Revised"))
            .unwrap();
        assert_eq!(updated.id, id);
        assert_eq!(store.drafts().len(), 1);
    }
    let mut store = MailStore::open(dir.path()).unwrap();
    assert_eq!(store.draft(&id).unwrap().subject, "Revised");
    assert_eq!(store.draft(&id).unwrap().body, "Hello 世界");
    assert_ne!(store.save_draft(input(None, "Next")).unwrap().id, id);
}

#[test]
fn unknown_ids_and_oversized_inputs_do_not_mutate_storage() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = MailStore::open(dir.path()).unwrap();
    assert!(
        store
            .save_draft(input(Some("missing".into()), "Oops"))
            .is_err()
    );
    assert!(store.save_draft(input(None, &"s".repeat(1000))).is_err());
    assert!(store.drafts().is_empty());
    // Empty recipients/subjects are valid unfinished drafts.
    assert!(
        store
            .save_draft(DraftInput {
                id: None,
                recipient: String::new(),
                subject: String::new(),
                body: String::new()
            })
            .is_ok()
    );
}

#[test]
fn corrupt_or_newer_stores_are_never_replaced() {
    for content in [
        "broken",
        r#"{"version":99,"drafts":[]}"#,
        r#"{"version":1,"drafts":[{"id":"draft-1","recipient":"","subject":"","body":""},{"id":"draft-1","recipient":"","subject":"","body":""}]}"#,
    ] {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("drafts.json"), content).unwrap();
        assert!(MailStore::open(dir.path()).is_err());
        assert_eq!(
            fs::read_to_string(dir.path().join("drafts.json")).unwrap(),
            content
        );
    }
}

#[test]
fn failed_persistence_keeps_previous_memory_and_disk() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = MailStore::open(dir.path()).unwrap();
    let draft = store.save_draft(input(None, "Saved")).unwrap();
    let file = dir.path().join("drafts.json");
    fs::rename(&file, dir.path().join("backup.json")).unwrap();
    fs::create_dir(&file).unwrap();
    assert!(
        store
            .save_draft(input(Some(draft.id.clone()), "Unsaved"))
            .is_err()
    );
    assert_eq!(store.draft(&draft.id).unwrap().subject, "Saved");
    fs::remove_dir(file).unwrap();
    fs::rename(
        dir.path().join("backup.json"),
        dir.path().join("drafts.json"),
    )
    .unwrap();
    drop(store);
    assert_eq!(
        MailStore::open(dir.path())
            .unwrap()
            .draft(&draft.id)
            .unwrap()
            .subject,
        "Saved"
    );
}

#[test]
fn only_one_store_can_write_a_directory() {
    let dir = tempfile::tempdir().unwrap();
    let first = MailStore::open(dir.path()).unwrap();
    assert!(MailStore::open(dir.path()).is_err());
    drop(first);
    assert!(MailStore::open(dir.path()).is_ok());
}
