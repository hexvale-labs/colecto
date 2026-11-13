use colecto_core::{Accounts, Database, Error, MailStore, SecretStore};
use colecto_extension_api::{AccountInput, DraftInput};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
#[derive(Default)]
struct MemorySecrets(Mutex<BTreeMap<String, String>>);
impl SecretStore for MemorySecrets {
    fn get(&self, id: &str) -> Result<String, Error> {
        self.0
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or(Error::Missing)
    }
    fn set(&self, id: &str, value: &str) -> Result<(), Error> {
        self.0.lock().unwrap().insert(id.into(), value.into());
        Ok(())
    }
    fn delete(&self, id: &str) -> Result<(), Error> {
        self.0.lock().unwrap().remove(id);
        Ok(())
    }
}

fn vault(dir: &std::path::Path) -> Vec<u8> {
    Database::open(dir)
        .unwrap()
        .get_record("core", "vault", 1)
        .unwrap()
        .unwrap()
}
fn write_vault(dir: &std::path::Path, bytes: Vec<u8>) -> Result<(), Error> {
    Database::open(dir)?.set_record("core", "vault", 1, &bytes)
}

fn input(id: Option<String>, address: &str) -> AccountInput {
    serde_json::from_value(serde_json::json!({"id":id,"name":address,"address":address,"protocol":"imap","imap_host":"imap.example.test","imap_port":993,"smtp_host":"smtp.example.test","smtp_port":587,"username":address,"secret":"test-secret"})).unwrap()
}

#[test]
fn account_stores_and_restart_are_isolated() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecrets::default());
    let mut accounts =
        Accounts::open(MailStore::open(dir.path()).unwrap(), secrets.clone()).unwrap();
    let a = accounts.upsert(input(None, "a@example.test")).unwrap();
    let b = accounts.upsert(input(None, "b@example.test")).unwrap();
    let draft = DraftInput {
        id: None,
        recipient: String::new(),
        subject: "A only".into(),
        body: String::new(),
        ..Default::default()
    };
    accounts
        .store(&a.id)
        .unwrap()
        .lock()
        .unwrap()
        .save_draft(draft)
        .unwrap();
    assert!(
        accounts
            .store(&b.id)
            .unwrap()
            .lock()
            .unwrap()
            .drafts()
            .is_empty()
    );
    assert!(accounts.store("../escape").is_err());
    drop(accounts);
    let accounts = Accounts::open(MailStore::open(dir.path()).unwrap(), secrets).unwrap();
    assert_eq!(
        accounts.store(&a.id).unwrap().lock().unwrap().drafts()[0].subject,
        "A only"
    );
    assert!(
        accounts
            .store(&b.id)
            .unwrap()
            .lock()
            .unwrap()
            .drafts()
            .is_empty()
    );
    assert!(
        !String::from_utf8(vault(dir.path()))
            .unwrap()
            .contains("test-secret")
    );
}

#[test]
fn password_vault_locks_restarts_and_rejects_wrong_password() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecrets::default());
    let mut accounts =
        Accounts::open(MailStore::open(dir.path()).unwrap(), secrets.clone()).unwrap();
    let a = accounts.upsert(input(None, "a@example.test")).unwrap();
    accounts.protect("a strong master password").unwrap();
    accounts.lock().unwrap();
    assert!(accounts.cached(&a.id, "inbox").unwrap().messages.is_empty());
    assert!(
        accounts
            .cached("demo", "inbox")
            .unwrap()
            .messages
            .is_empty()
    );
    assert!(!accounts.store(&a.id).unwrap().lock().unwrap().connected());
    assert!(accounts.unlock("wrong password").is_err());
    assert!(accounts.security().locked);
    accounts.unlock("a strong master password").unwrap();
    assert!(accounts.store(&a.id).unwrap().lock().unwrap().connected());
    drop(accounts);
    let mut accounts = Accounts::open(MailStore::open(dir.path()).unwrap(), secrets).unwrap();
    assert!(accounts.security().locked);
    accounts.unlock("a strong master password").unwrap();
    accounts.unprotect().unwrap();
    assert!(!accounts.security().protected);
}

#[test]
fn busy_account_does_not_block_other_accounts_or_allow_lock() {
    let dir = tempfile::tempdir().unwrap();
    let mut accounts = Accounts::open(
        MailStore::open(dir.path()).unwrap(),
        Arc::new(MemorySecrets::default()),
    )
    .unwrap();
    let a = accounts.upsert(input(None, "a@example.test")).unwrap();
    let b = accounts.upsert(input(None, "b@example.test")).unwrap();
    accounts.protect("a strong master password").unwrap();
    let handle = accounts.store(&a.id).unwrap();
    let _operation = handle.lock().unwrap();
    assert!(accounts.store(&b.id).unwrap().try_lock().is_ok());
    assert!(accounts.lock().is_err());
    assert!(!accounts.security().locked);
}

#[test]
fn tampered_vault_is_not_overwritten_and_reset_keeps_drafts() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecrets::default());
    let mut accounts =
        Accounts::open(MailStore::open(dir.path()).unwrap(), secrets.clone()).unwrap();
    let a = accounts.upsert(input(None, "a@example.test")).unwrap();
    accounts
        .store(&a.id)
        .unwrap()
        .lock()
        .unwrap()
        .save_draft(DraftInput {
            id: None,
            recipient: String::new(),
            subject: "Keep me".into(),
            body: String::new(),
            ..Default::default()
        })
        .unwrap();
    accounts.protect("a strong master password").unwrap();
    drop(accounts);
    let mut value: serde_json::Value = serde_json::from_slice(&vault(dir.path())).unwrap();
    value["mode"]["ciphertext"][0] =
        serde_json::json!(value["mode"]["ciphertext"][0].as_u64().unwrap() ^ 1);
    write_vault(dir.path(), serde_json::to_vec(&value).unwrap()).unwrap();
    let before = vault(dir.path());
    let mut accounts = Accounts::open(MailStore::open(dir.path()).unwrap(), secrets).unwrap();
    assert!(accounts.unlock("a strong master password").is_err());
    assert_eq!(vault(dir.path()), before);
    accounts.reset().unwrap();
    assert_eq!(
        accounts.store(&a.id).unwrap().lock().unwrap().drafts()[0].subject,
        "Keep me"
    );
    assert!(
        !accounts
            .list()
            .iter()
            .find(|r| r.id == a.id)
            .unwrap()
            .configured
    );
}

#[test]
fn system_backend_failure_has_no_plaintext_fallback() {
    struct Unavailable;
    impl SecretStore for Unavailable {
        fn get(&self, _: &str) -> Result<String, Error> {
            Err(Error::Invalid("keychain locked".into()))
        }
        fn set(&self, _: &str, _: &str) -> Result<(), Error> {
            Err(Error::Invalid("keychain locked".into()))
        }
        fn delete(&self, _: &str) -> Result<(), Error> {
            Ok(())
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let mut accounts =
        Accounts::open(MailStore::open(dir.path()).unwrap(), Arc::new(Unavailable)).unwrap();
    assert!(accounts.upsert(input(None, "a@example.test")).is_err());
    assert_eq!(accounts.list().len(), 1);
    assert!(
        !String::from_utf8(vault(dir.path()))
            .unwrap()
            .contains("test-secret")
    );
    // Password protection works independently of a missing desktop keychain.
    accounts.protect("a strong master password").unwrap();
    assert!(accounts.upsert(input(None, "a@example.test")).is_ok());
    assert!(accounts.unprotect().is_err());
    assert!(accounts.security().protected);
}

#[test]
fn metadata_tampering_cannot_redirect_a_protected_credential() {
    let dir = tempfile::tempdir().unwrap();
    let backend = Arc::new(MemorySecrets::default());
    let mut accounts =
        Accounts::open(MailStore::open(dir.path()).unwrap(), backend.clone()).unwrap();
    accounts.upsert(input(None, "a@example.test")).unwrap();
    accounts.protect("a strong master password").unwrap();
    drop(accounts);
    let mut state: serde_json::Value = serde_json::from_slice(&vault(dir.path())).unwrap();
    state["accounts"][0]["settings"]["imap_host"] = serde_json::json!("attacker.example.test");
    write_vault(dir.path(), serde_json::to_vec(&state).unwrap()).unwrap();
    let mut accounts = Accounts::open(MailStore::open(dir.path()).unwrap(), backend).unwrap();
    assert!(accounts.unlock("a strong master password").is_err());
}

#[test]
fn startup_cleans_journaled_orphan_without_losing_active_credentials() {
    let dir = tempfile::tempdir().unwrap();
    let backend = Arc::new(MemorySecrets::default());
    let mut accounts =
        Accounts::open(MailStore::open(dir.path()).unwrap(), backend.clone()).unwrap();
    let a = accounts.upsert(input(None, "a@example.test")).unwrap();
    drop(accounts);
    let mut state: serde_json::Value = serde_json::from_slice(&vault(dir.path())).unwrap();
    state["cleanup"] = serde_json::json!(["00000000000000000000000000000000"]);
    backend
        .set("00000000000000000000000000000000", "orphan secret")
        .unwrap();
    write_vault(dir.path(), serde_json::to_vec(&state).unwrap()).unwrap();
    let accounts = Accounts::open(MailStore::open(dir.path()).unwrap(), backend.clone()).unwrap();
    assert!(backend.get("00000000000000000000000000000000").is_err());
    assert!(accounts.store(&a.id).unwrap().lock().unwrap().connected());
}

#[test]
fn failed_cleanup_retains_encrypted_replacement_and_recovers_on_restart() {
    use std::sync::atomic::{AtomicBool, Ordering};
    #[derive(Default)]
    struct Flaky {
        memory: MemorySecrets,
        fail_delete: AtomicBool,
    }
    impl SecretStore for Flaky {
        fn get(&self, id: &str) -> Result<String, Error> {
            self.memory.get(id)
        }
        fn set(&self, id: &str, value: &str) -> Result<(), Error> {
            self.memory.set(id, value)
        }
        fn delete(&self, id: &str) -> Result<(), Error> {
            if self.fail_delete.load(Ordering::SeqCst) {
                Err(Error::Invalid("keychain locked".into()))
            } else {
                self.memory.delete(id)
            }
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let backend = Arc::new(Flaky::default());
    let mut accounts =
        Accounts::open(MailStore::open(dir.path()).unwrap(), backend.clone()).unwrap();
    let a = accounts.upsert(input(None, "a@example.test")).unwrap();
    backend.fail_delete.store(true, Ordering::SeqCst);
    assert!(accounts.protect("a strong master password").is_err());
    assert!(accounts.security().protected);
    assert!(accounts.security().cleanup_pending);
    drop(accounts);
    backend.fail_delete.store(false, Ordering::SeqCst);
    let mut accounts =
        Accounts::open(MailStore::open(dir.path()).unwrap(), backend.clone()).unwrap();
    assert!(backend.memory.0.lock().unwrap().is_empty());
    accounts.unlock("a strong master password").unwrap();
    assert!(accounts.store(&a.id).unwrap().lock().unwrap().connected());
}

#[test]
fn locked_vault_suppresses_legacy_environment_account() {
    let dir = tempfile::tempdir().unwrap();
    let backend = Arc::new(MemorySecrets::default());
    let mut accounts =
        Accounts::open(MailStore::open(dir.path()).unwrap(), backend.clone()).unwrap();
    accounts.protect("a strong master password").unwrap();
    drop(accounts);
    let mut legacy = MailStore::open(dir.path()).unwrap();
    legacy
        .connect_account(colecto_core::MailAccount {
            imap_host: "imap.example.test".into(),
            imap_port: 993,
            smtp_host: "smtp.example.test".into(),
            smtp_port: 587,
            username: "env".into(),
            password: "must-not-bypass-lock".into(),
            address: "env@example.test".into(),
            imap_starttls: false,
            tls_certificate: String::new(),
        })
        .unwrap();
    let accounts = Accounts::open(legacy, backend).unwrap();
    assert!(accounts.security().locked);
    assert!(!accounts.store("demo").unwrap().lock().unwrap().connected());
}

#[test]
fn corrupt_cleanup_never_deletes_active_system_entry() {
    let dir = tempfile::tempdir().unwrap();
    let backend = Arc::new(MemorySecrets::default());
    let mut accounts =
        Accounts::open(MailStore::open(dir.path()).unwrap(), backend.clone()).unwrap();
    accounts.upsert(input(None, "a@example.test")).unwrap();
    drop(accounts);
    let mut state: serde_json::Value = serde_json::from_slice(&vault(dir.path())).unwrap();
    let active = state["mode"]["entry"].as_str().unwrap().to_string();
    state["cleanup"] = serde_json::json!([active]);
    write_vault(dir.path(), serde_json::to_vec(&state).unwrap()).unwrap();
    let before = vault(dir.path());
    assert!(Accounts::open(MailStore::open(dir.path()).unwrap(), backend.clone()).is_err());
    assert!(backend.get(&active).is_ok());
    assert_eq!(vault(dir.path()), before);
}

#[test]
fn long_credentials_fit_windows_system_entry_limit() {
    let dir = tempfile::tempdir().unwrap();
    let backend = Arc::new(MemorySecrets::default());
    let mut accounts =
        Accounts::open(MailStore::open(dir.path()).unwrap(), backend.clone()).unwrap();
    let mut account = input(None, "a@example.test");
    account.secret = "x".repeat(3000);
    accounts.upsert(account).unwrap();
    assert!(
        backend
            .0
            .lock()
            .unwrap()
            .values()
            .all(|value| value.encode_utf16().count() * 2 <= 2560)
    );
}

#[test]
fn optional_profile_name_is_preserved_and_can_be_set() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecrets::default());
    let mut accounts =
        Accounts::open(MailStore::open(dir.path()).unwrap(), secrets.clone()).unwrap();
    let mut account = input(None, "me@example.test");
    account.settings.name = "  ".into();
    let saved = accounts.upsert(account).unwrap();
    assert!(saved.settings.as_ref().unwrap().name.is_empty());
    let mut update = input(Some(saved.id.clone()), "me@example.test");
    update.settings.name = "  Personal  ".into();
    accounts.upsert(update).unwrap();
    drop(accounts);
    let accounts = Accounts::open(MailStore::open(dir.path()).unwrap(), secrets).unwrap();
    assert_eq!(
        accounts
            .list()
            .iter()
            .find(|a| a.id == saved.id)
            .unwrap()
            .settings
            .as_ref()
            .unwrap()
            .name,
        "Personal"
    );
}

#[test]
fn address_longer_than_profile_limit_can_be_edited_without_a_profile_name() {
    let dir = tempfile::tempdir().unwrap();
    let secrets = Arc::new(MemorySecrets::default());
    let mut accounts = Accounts::open(MailStore::open(dir.path()).unwrap(), secrets).unwrap();
    let address = format!("{}@{}.example.test", "a".repeat(60), "b".repeat(50));
    let mut new = input(None, &address);
    new.settings.name.clear();
    let saved = accounts.upsert(new).unwrap();
    let mut edit = input(Some(saved.id.clone()), &address);
    edit.settings = saved.settings.unwrap();
    assert!(accounts.upsert(edit).is_ok());
}

#[test]
fn cached_list_and_reader_remain_available_while_account_is_locked() {
    let dir = tempfile::tempdir().unwrap();
    let accounts = Accounts::open(
        MailStore::open(dir.path()).unwrap(),
        Arc::new(MemorySecrets::default()),
    )
    .unwrap();
    let handle = accounts.store("demo").unwrap();
    let _network_work = handle.lock().unwrap();
    let snapshot = accounts.cached("demo", "inbox").unwrap();
    assert_eq!(snapshot.messages.len(), 4);
    assert!(
        accounts
            .cached_message("demo", &snapshot.messages[0].id)
            .unwrap()
            .is_some()
    );
    assert!(accounts.cached("other", "inbox").is_err());
}

#[test]
fn fresh_vault_ignores_legacy_account_registry() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("accounts.json"),
        b"unsupported legacy content",
    )
    .unwrap();
    let accounts = Accounts::open(
        MailStore::open(dir.path()).unwrap(),
        Arc::new(MemorySecrets::default()),
    )
    .unwrap();
    assert!(accounts.store("unknown").is_err());
    let state: serde_json::Value = serde_json::from_slice(&vault(dir.path())).unwrap();
    assert!(state["accounts"].as_array().unwrap().is_empty());
}
