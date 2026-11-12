//! Account ownership and isolated stores. Network work uses account-local locks.
use crate::{
    Account, Error, JmapAccount, MailAccount, MailStore,
    credentials::{Secret, SecretStore, Vault, identifier},
};
use colecto_extension_api::{
    AccountInfo, AccountInput, AccountSettings, DEMO_ACCOUNT, Protocol, SecurityInfo,
};
use std::{
    collections::{BTreeMap, HashSet},
    sync::{Arc, Mutex},
};

pub struct Accounts {
    vault: Vault,
    database: crate::Database,
    directory: std::path::PathBuf,
    stores: BTreeMap<String, Arc<Mutex<MailStore>>>,
    snapshots: BTreeMap<String, crate::SnapshotHandle>,
}
fn invalid(message: &str) -> Error {
    Error::Invalid(message.into())
}
fn busy() -> Error {
    invalid("Mail operation in progress. Try again when it finishes.")
}
fn prepare_connection(
    store: &mut MailStore,
    settings: &AccountSettings,
    secret: &str,
) -> Result<crate::PreparedConnection, Error> {
    let account = match settings.protocol {
        Protocol::Imap => Account::Imap(MailAccount {
            imap_host: settings.imap_host.clone(),
            imap_port: settings.imap_port,
            smtp_host: settings.smtp_host.clone(),
            smtp_port: settings.smtp_port,
            username: settings.username.clone(),
            password: secret.into(),
            address: settings.address.clone(),
        }),
        Protocol::Jmap => Account::Jmap(JmapAccount {
            session_url: settings.session_url.clone(),
            token: secret.into(),
            address: settings.address.clone(),
            username: settings.username.clone(),
        }),
    };
    store.prepare_connection(account)
}
fn connect(store: &mut MailStore, settings: &AccountSettings, secret: &str) -> Result<(), Error> {
    let prepared = prepare_connection(store, settings, secret)?;
    store.apply_connection(prepared);
    Ok(())
}

impl Accounts {
    /// Host-owned data, including extension policy, lives beside account stores.
    pub fn database(&self) -> crate::Database {
        self.database.clone()
    }

    pub fn data_directory(&self) -> &std::path::Path {
        &self.directory
    }

    pub fn open(mut legacy: MailStore, backend: Arc<dyn SecretStore>) -> Result<Self, Error> {
        let directory = legacy.directory.clone();
        let database = legacy.database.clone();
        let mut vault = Vault::open(directory.clone(), database.clone(), backend)?;
        let mut ids = HashSet::new();
        for account in vault.accounts() {
            if (account.id != DEMO_ACCOUNT
                && (account.id.len() != 32 || !account.id.bytes().all(|b| b.is_ascii_hexdigit())))
                || !ids.insert(account.id.clone())
            {
                return Err(invalid("Invalid or duplicate account identifier"));
            }
        }
        // Import legacy environment credentials only when unlocked. The legacy
        // draft store keeps its identity and is never assigned to another account.
        if !vault.locked() && !ids.contains(DEMO_ACCOUNT) {
            let input = match &legacy.account {
                Some(Account::Imap(a)) => Some(AccountInput {
                    id: Some(DEMO_ACCOUNT.into()),
                    settings: AccountSettings {
                        name: a.address.clone(),
                        address: a.address.clone(),
                        protocol: Protocol::Imap,
                        imap_host: a.imap_host.clone(),
                        imap_port: a.imap_port,
                        smtp_host: a.smtp_host.clone(),
                        smtp_port: a.smtp_port,
                        username: a.username.clone(),
                        session_url: String::new(),
                    },
                    secret: a.password.clone(),
                }),
                Some(Account::Jmap(a)) => Some(AccountInput {
                    id: Some(DEMO_ACCOUNT.into()),
                    settings: AccountSettings {
                        name: a.address.clone(),
                        address: a.address.clone(),
                        protocol: Protocol::Jmap,
                        imap_host: String::new(),
                        imap_port: 993,
                        smtp_host: String::new(),
                        smtp_port: 587,
                        username: a.username.clone(),
                        session_url: a.session_url.clone(),
                    },
                    secret: a.token.clone(),
                }),
                None => None,
            };
            if let Some(mut input) = input {
                let mut records = vault.accounts().to_vec();
                records.push(crate::credentials::SavedAccount {
                    id: DEMO_ACCOUNT.into(),
                    settings: input.settings.clone(),
                    configured: true,
                });
                let mut secrets = vault.secrets.as_ref().expect("unlocked").clone();
                secrets.insert(
                    DEMO_ACCOUNT.into(),
                    Secret(std::mem::take(&mut input.secret)),
                );
                vault.commit(records, secrets)?;
            }
        }
        if vault.locked() {
            legacy.disconnect();
        }
        let mut stores = BTreeMap::new();
        stores.insert(DEMO_ACCOUNT.into(), Arc::new(Mutex::new(legacy)));
        for account in vault.accounts() {
            if account.id != DEMO_ACCOUNT {
                stores.insert(
                    account.id.clone(),
                    Arc::new(Mutex::new(MailStore::open_with_database(
                        directory.join("accounts").join(&account.id),
                        database.clone(),
                        &account.id,
                    )?)),
                );
            }
        }
        let snapshots = stores
            .iter()
            .map(|(id, store)| (id.clone(), store.lock().unwrap().snapshot_handle()))
            .collect();
        let mut accounts = Self {
            snapshots,
            database,
            vault,
            stores,
            directory,
        };
        accounts.reconnect()?;
        Ok(accounts)
    }
    fn reconnect(&mut self) -> Result<(), Error> {
        for (id, store) in &self.stores {
            let mut store = store.try_lock().map_err(|_| busy())?;
            if let Some(record) = self.vault.accounts().iter().find(|a| &a.id == id) {
                store.disconnect();
                if record.configured
                    && let Some(secret) = self.vault.secrets.as_ref().and_then(|s| s.get(id))
                {
                    connect(&mut store, &record.settings, &secret.0)?;
                }
            } else if self.vault.locked() {
                store.disconnect();
            }
        }
        Ok(())
    }
    pub fn list(&self) -> Vec<AccountInfo> {
        self.stores
            .iter()
            .map(|(id, store)| {
                let record = self.vault.accounts().iter().find(|a| &a.id == id);
                AccountInfo {
                    id: id.clone(),
                    settings: record.map(|a| a.settings.clone()),
                    configured: record.is_some_and(|a| a.configured),
                    connected: store.try_lock().map(|s| s.connected()).unwrap_or_else(|_| {
                        !self.vault.locked() && record.is_some_and(|a| a.configured)
                    }),
                }
            })
            .collect()
    }
    pub fn cached_message(
        &self,
        id: &str,
        message_id: &str,
    ) -> Result<Option<colecto_extension_api::Message>, Error> {
        let snapshots = self
            .snapshots
            .get(id)
            .ok_or(Error::Missing)?
            .read()
            .map_err(crate::storage)?;
        Ok(snapshots
            .values()
            .flat_map(|s| &s.messages)
            .find(|m| m.id == message_id)
            .cloned())
    }
    pub fn cached(&self, id: &str, folder: &str) -> Result<crate::MailSnapshot, Error> {
        crate::validate_query(folder, "")?;
        let snapshot = self
            .snapshots
            .get(id)
            .ok_or(Error::Missing)?
            .read()
            .map_err(crate::storage)?;
        Ok(snapshot
            .get(folder)
            .cloned()
            .unwrap_or_else(|| crate::MailSnapshot {
                folder: folder.into(),
                ..Default::default()
            }))
    }
    pub fn store(&self, id: &str) -> Result<Arc<Mutex<MailStore>>, Error> {
        self.stores.get(id).cloned().ok_or(Error::Missing)
    }
    pub fn upsert(&mut self, mut input: AccountInput) -> Result<AccountInfo, Error> {
        let secret = Secret(std::mem::take(&mut input.secret));
        let mut settings = input.settings;
        settings.name = settings.name.trim().to_owned();
        settings.address = settings.address.trim().to_owned();
        if settings.name.len() > 100
            || settings.address.len() > 320
            || settings.imap_host.len() > 253
            || settings.smtp_host.len() > 253
            || settings.username.len() > 320
            || settings.session_url.len() > 2048
            || secret.0.len() > 8192
        {
            return Err(invalid("Check account name, address and server settings"));
        }
        let id = input.id.unwrap_or_else(identifier);
        let existing = self.vault.accounts().iter().find(|a| a.id == id);
        if self.stores.contains_key(&id) && existing.is_none() {
            return Err(invalid(
                "The sample account cannot be replaced. Add a new account instead.",
            ));
        }
        if existing.is_none() && self.vault.accounts().len() >= 50 {
            return Err(invalid("Account limit reached (50)"));
        }
        if existing.is_none() && (id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit())) {
            return Err(invalid("Invalid account identifier"));
        }
        let mut secrets = self
            .vault
            .secrets
            .as_ref()
            .ok_or_else(|| invalid("Unlock credentials first"))?
            .clone();
        let secret = if secret.0.is_empty() {
            secrets
                .get(&id)
                .cloned()
                .ok_or_else(|| invalid("Enter a password or token"))?
        } else {
            secret
        };
        if self
            .vault
            .accounts()
            .iter()
            .any(|a| a.id != id && a.settings.address.eq_ignore_ascii_case(&settings.address))
        {
            return Err(invalid(
                "This email address already has an account. Reconnect it instead.",
            ));
        }
        let handle = match self.stores.get(&id) {
            Some(store) => store.clone(),
            None => Arc::new(Mutex::new(MailStore::open_with_database(
                self.directory.clone().join("accounts").join(&id),
                self.database.clone(),
                &id,
            )?)),
        };
        let mut store = handle.try_lock().map_err(|_| busy())?;
        // Validate without changing the active transport before persistence.
        let prepared = prepare_connection(&mut store, &settings, &secret.0)?;
        let mut records = self.vault.accounts().to_vec();
        records.retain(|a| a.id != id);
        records.push(crate::credentials::SavedAccount {
            id: id.clone(),
            settings: settings.clone(),
            configured: true,
        });
        secrets.insert(id.clone(), secret.clone());
        let result = self.vault.commit(records, secrets);
        if self
            .vault
            .accounts()
            .iter()
            .any(|a| a.id == id && a.configured && a.settings == settings)
            && self
                .vault
                .secrets
                .as_ref()
                .and_then(|s| s.get(&id))
                .is_some_and(|s| s.0 == secret.0)
        {
            store.apply_connection(prepared);
            self.snapshots.insert(id.clone(), store.snapshot_handle());
            drop(store);
            self.stores.insert(id.clone(), handle);
        }
        result?;
        Ok(self
            .list()
            .into_iter()
            .find(|a| a.id == id)
            .expect("stored account"))
    }
    pub fn disconnect(&mut self, id: &str) -> Result<(), Error> {
        let handle = self.store(id)?;
        let mut store = handle.try_lock().map_err(|_| busy())?;
        let mut records = self.vault.accounts().to_vec();
        let record = records
            .iter_mut()
            .find(|a| a.id == id)
            .ok_or_else(|| invalid("Sample account has no credentials"))?;
        let mut secrets = self
            .vault
            .secrets
            .as_ref()
            .ok_or_else(|| invalid("Unlock credentials first"))?
            .clone();
        record.configured = false;
        secrets.remove(id);
        let result = self.vault.commit(records, secrets);
        if self
            .vault
            .accounts()
            .iter()
            .any(|a| a.id == id && !a.configured)
        {
            store.disconnect();
        }
        result
    }
    pub fn security(&self) -> SecurityInfo {
        SecurityInfo {
            cleanup_pending: self.vault.cleanup_pending(),
            protected: self.vault.protected(),
            locked: self.vault.locked(),
        }
    }
    pub fn protect(&mut self, password: &str) -> Result<(), Error> {
        let handles: Vec<_> = self.stores.values().cloned().collect();
        let _guards: Vec<_> = handles
            .iter()
            .map(|s| s.try_lock().map_err(|_| busy()))
            .collect::<Result<_, _>>()?;
        self.vault.protect(password)
    }
    pub fn unprotect(&mut self) -> Result<(), Error> {
        let handles: Vec<_> = self.stores.values().cloned().collect();
        let _guards: Vec<_> = handles
            .iter()
            .map(|s| s.try_lock().map_err(|_| busy()))
            .collect::<Result<_, _>>()?;
        self.vault.unprotect()
    }
    pub fn lock(&mut self) -> Result<(), Error> {
        let handles: Vec<_> = self.stores.values().cloned().collect();
        let mut guards: Vec<_> = handles
            .iter()
            .map(|s| s.try_lock().map_err(|_| busy()))
            .collect::<Result<_, _>>()?;
        self.vault.lock()?;
        for store in &mut guards {
            store.disconnect();
        }
        Ok(())
    }
    pub fn unlock(&mut self, password: &str) -> Result<(), Error> {
        let entries: Vec<_> = self
            .stores
            .iter()
            .map(|(id, store)| (id.clone(), store.clone()))
            .collect();
        let mut guards: Vec<_> = entries
            .iter()
            .map(|(_, store)| store.try_lock().map_err(|_| busy()))
            .collect::<Result<_, _>>()?;
        self.vault.unlock(password)?;
        let result = entries
            .iter()
            .zip(guards.iter_mut())
            .try_for_each(|((id, _), store)| {
                if let Some(record) = self.vault.accounts().iter().find(|a| &a.id == id) {
                    store.disconnect();
                    if record.configured
                        && let Some(secret) = self.vault.secrets.as_ref().and_then(|s| s.get(id))
                    {
                        connect(store, &record.settings, &secret.0)?;
                    }
                }
                Ok(())
            });
        if result.is_err() {
            for store in &mut guards {
                store.disconnect();
            }
            self.vault.lock()?;
        }
        result
    }
    pub fn cleanup(&mut self) -> Result<(), Error> {
        self.vault.cleanup()
    }
    pub fn reset(&mut self) -> Result<(), Error> {
        let handles: Vec<_> = self.stores.values().cloned().collect();
        let mut guards: Vec<_> = handles
            .iter()
            .map(|s| s.try_lock().map_err(|_| busy()))
            .collect::<Result<_, _>>()?;
        let result = self.vault.reset();
        if !self.vault.protected() {
            for store in &mut guards {
                store.disconnect();
            }
        }
        result
    }
}

#[cfg(test)]
mod storage_tests {
    use super::*;
    #[test]
    fn vault_lock_revokes_detached_push_credentials() {
        struct NoKeys;
        impl SecretStore for NoKeys {
            fn get(&self, _: &str) -> Result<String, Error> {
                Err(Error::Missing)
            }
            fn set(&self, _: &str, _: &str) -> Result<(), Error> {
                Ok(())
            }
            fn delete(&self, _: &str) -> Result<(), Error> {
                Ok(())
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let mut accounts =
            Accounts::open(MailStore::open(dir.path()).unwrap(), Arc::new(NoKeys)).unwrap();
        accounts.protect("a strong test password").unwrap();
        let store = accounts.store("demo").unwrap();
        let watcher = {
            let mut store = store.lock().unwrap();
            store.account = Some(Account::Jmap(JmapAccount {
                session_url: "https://example.test/jmap".into(),
                token: "secret".into(),
                address: "me@example.test".into(),
                username: String::new(),
            }));
            store.account_key = Some("test".into());
            store.push_watcher().unwrap()
        };
        accounts.lock().unwrap();
        assert!(watcher.account.lock().unwrap().is_none());
        assert!(matches!(
            watcher.wait(std::time::Duration::ZERO),
            Err(Error::Missing)
        ));
    }

    #[test]
    fn failed_registry_transaction_never_stages_a_system_key() {
        struct NoKeys;
        impl SecretStore for NoKeys {
            fn get(&self, _: &str) -> Result<String, Error> {
                panic!("No key should exist")
            }
            fn set(&self, _: &str, _: &str) -> Result<(), Error> {
                panic!("Failed database write must precede keyring writes")
            }
            fn delete(&self, _: &str) -> Result<(), Error> {
                panic!("No key should exist")
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let mut accounts =
            Accounts::open(MailStore::open(dir.path()).unwrap(), Arc::new(NoKeys)).unwrap();
        let before = accounts.database.get_record("core", "vault", 1).unwrap();
        futures_executor::block_on(
            accounts
                .database
                .connection()
                .unwrap()
                .execute("PRAGMA query_only=1", ()),
        )
        .unwrap();
        let input = serde_json::from_value(serde_json::json!({"id":null,"name":"A","address":"a@example.test","protocol":"imap","imap_host":"imap.example.test","imap_port":993,"smtp_host":"smtp.example.test","smtp_port":587,"username":"a","secret":"secret"})).unwrap();
        assert!(accounts.upsert(input).is_err());
        assert_eq!(
            accounts.database.get_record("core", "vault", 1).unwrap(),
            before
        );
    }
}
