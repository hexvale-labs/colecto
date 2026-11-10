//! Credential persistence; plaintext is never written to the account registry.
use crate::{Database, Error, storage};
use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, AeadCore, KeyInit, OsRng, Payload, rand_core::RngCore},
};
use colecto_extension_api::AccountSettings;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::PathBuf,
    sync::Arc,
};
use zeroize::{Zeroize, ZeroizeOnDrop, Zeroizing};

pub trait SecretStore: Send + Sync {
    fn get(&self, id: &str) -> Result<String, Error>;
    fn set(&self, id: &str, value: &str) -> Result<(), Error>;
    fn delete(&self, id: &str) -> Result<(), Error>;
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SavedAccount {
    pub id: String,
    pub settings: AccountSettings,
    pub configured: bool,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
enum Mode {
    System {
        entry: Option<String>,
        nonce: [u8; 24],
        ciphertext: Vec<u8>,
    },
    Password {
        salt: [u8; 16],
        nonce: [u8; 24],
        ciphertext: Vec<u8>,
    },
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct State {
    version: u32,
    accounts: Vec<SavedAccount>,
    mode: Mode,
    cleanup: Vec<String>,
}
#[derive(Clone, Serialize, Deserialize, Zeroize, ZeroizeOnDrop)]
pub(crate) struct Secret(pub String);
pub(crate) type Secrets = BTreeMap<String, Secret>;

pub(crate) struct Vault {
    directory: PathBuf,
    database: Database,
    state: State,
    backend: Arc<dyn SecretStore>,
    pub secrets: Option<Secrets>,
    key: Option<Zeroizing<[u8; 32]>>,
}
fn invalid(message: &str) -> Error {
    Error::Invalid(message.into())
}
fn key(password: &str, salt: &[u8; 16]) -> Result<Zeroizing<[u8; 32]>, Error> {
    let mut key = Zeroizing::new([0; 32]);
    // Format v1 fixes Argon2id v19 at 64 MiB, 3 iterations, one lane.
    let params = Params::new(65536, 3, 1, Some(32)).map_err(storage)?;
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(password.as_bytes(), salt, key.as_mut())
        .map_err(storage)?;
    Ok(key)
}
fn seal(
    secrets: &Secrets,
    accounts: &[SavedAccount],
    key: &[u8; 32],
    salt: [u8; 16],
) -> Result<Mode, Error> {
    let plaintext = Zeroizing::new(serde_json::to_vec(secrets).map_err(storage)?);
    let cipher = XChaCha20Poly1305::new(key.into());
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let aad = serde_json::to_vec(accounts).map_err(storage)?;
    let ciphertext = cipher
        .encrypt(
            &nonce,
            Payload {
                msg: plaintext.as_slice(),
                aad: &aad,
            },
        )
        .map_err(|_| invalid("Could not encrypt credentials"))?;
    Ok(Mode::Password {
        salt,
        nonce: nonce.into(),
        ciphertext,
    })
}
pub(crate) fn identifier() -> String {
    let mut bytes = [0u8; 16];
    OsRng.fill_bytes(&mut bytes);
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
impl Vault {
    pub fn open(
        directory: PathBuf,
        database: Database,
        backend: Arc<dyn SecretStore>,
    ) -> Result<Self, Error> {
        let canonical = database.contains_record("core", "vault")?;
        let bytes = if canonical {
            Some(
                database
                    .get_record("core", "vault", 1)?
                    .ok_or_else(|| storage("Authoritative credential vault was removed"))?,
            )
        } else {
            match fs::metadata(directory.join("accounts.json")) {
                Ok(metadata) if metadata.len() > 32_000_000 => {
                    return Err(storage("Account registry exceeds its size limit"));
                }
                Ok(_) => Some(fs::read(directory.join("accounts.json")).map_err(storage)?),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(storage(error)),
            }
        };
        let state: State = match bytes {
            Some(bytes) => serde_json::from_slice(&bytes).map_err(|_| {
                invalid("Invalid account registry; preserve accounts.json before recovery")
            })?,
            None => State {
                version: 1,
                accounts: vec![],
                mode: Mode::System {
                    entry: None,
                    nonce: [0; 24],
                    ciphertext: vec![],
                },
                cleanup: vec![],
            },
        };
        if state.version != 1 {
            return Err(invalid("Unsupported account registry version"));
        }
        let valid_id = |id: &str| id.len() == 32 && id.bytes().all(|b| b.is_ascii_hexdigit());
        let mut ids = HashSet::new();
        if state
            .accounts
            .iter()
            .any(|a| (a.id != "demo" && !valid_id(&a.id)) || !ids.insert(a.id.clone()))
        {
            return Err(invalid("Invalid or duplicate account identifier"));
        }
        let active = match &state.mode {
            Mode::System { entry, .. } => entry.as_deref(),
            _ => None,
        };
        let mut cleanup_ids = HashSet::new();
        if active.is_some_and(|id| !valid_id(id))
            || state
                .cleanup
                .iter()
                .any(|id| !valid_id(id) || Some(id.as_str()) == active || !cleanup_ids.insert(id))
        {
            return Err(invalid(
                "Invalid credential cleanup journal; preserve accounts.json before recovery",
            ));
        }
        if matches!(&state.mode,Mode::System {entry:None,ciphertext,..} if !ciphertext.is_empty()) {
            return Err(invalid("Invalid empty system credential vault"));
        }
        let mut vault = Self {
            directory,
            database,
            state,
            backend,
            secrets: None,
            key: None,
        };
        if let Mode::System {
            entry,
            nonce,
            ciphertext,
        } = &vault.state.mode
        {
            vault.secrets = Some(match entry {
                Some(id) => {
                    let key: Zeroizing<[u8; 32]> = Zeroizing::new(
                        serde_json::from_str(&Zeroizing::new(vault.backend.get(id)?))
                            .map_err(|_| invalid("Invalid system credential key"))?,
                    );
                    let aad = serde_json::to_vec(&vault.state.accounts).map_err(storage)?;
                    let plaintext = Zeroizing::new(
                        XChaCha20Poly1305::new((&*key).into())
                            .decrypt(
                                XNonce::from_slice(nonce),
                                Payload {
                                    msg: ciphertext,
                                    aad: &aad,
                                },
                            )
                            .map_err(|_| invalid("Damaged system credential vault"))?,
                    );
                    serde_json::from_slice(&plaintext)
                        .map_err(|_| invalid("Damaged system credential vault"))?
                }
                None => BTreeMap::new(),
            });
        }
        if !canonical {
            let serialized = serde_json::to_vec(&vault.state).map_err(storage)?;
            let canonical = vault
                .database
                .initialize_record("core", "vault", 1, &serialized)?;
            if canonical != serialized {
                return Self::open(
                    vault.directory.clone(),
                    vault.database.clone(),
                    vault.backend.clone(),
                );
            }
        }
        vault.cleanup()?;
        Ok(vault)
    }
    pub fn accounts(&self) -> &[SavedAccount] {
        &self.state.accounts
    }
    pub fn cleanup_pending(&self) -> bool {
        !self.state.cleanup.is_empty()
    }
    pub fn protected(&self) -> bool {
        matches!(self.state.mode, Mode::Password { .. })
    }
    pub fn locked(&self) -> bool {
        self.secrets.is_none()
    }
    fn write(&self, state: &State) -> Result<(), Error> {
        self.database.set_record(
            "core",
            "vault",
            1,
            &serde_json::to_vec(state).map_err(storage)?,
        )
    }

    pub fn cleanup(&mut self) -> Result<(), Error> {
        self.sync_directory()?;
        for id in &self.state.cleanup {
            self.backend.delete(id)?;
        }
        if !self.state.cleanup.is_empty() {
            let mut next = self.state.clone();
            next.cleanup.clear();
            self.write(&next)?;
            self.state = next;
        }
        Ok(())
    }
    fn persist(
        &mut self,
        accounts: Vec<SavedAccount>,
        secrets: Secrets,
        mode: Mode,
        next_key: Option<Zeroizing<[u8; 32]>>,
    ) -> Result<(), Error> {
        let active = match &mode {
            Mode::System { entry, .. } => entry.as_ref(),
            _ => None,
        };
        let cleanup = self
            .state
            .cleanup
            .iter()
            .filter(|id| Some(*id) != active)
            .cloned()
            .collect();
        let mut next = State {
            version: 1,
            accounts,
            mode,
            cleanup,
        };
        if let Mode::System {
            entry: Some(id), ..
        } = &self.state.mode
        {
            next.cleanup.push(id.clone());
        }
        if let Err(error) = self.write(&next) {
            let _ = self.cleanup();
            return Err(error);
        }
        self.state = next;
        self.secrets = Some(secrets);
        self.key = next_key;
        // The replacement is already authoritative if cleanup fails. Retry on the
        // next mutation/startup; never report protection complete prematurely.
        self.cleanup()
    }
    fn sync_directory(&self) -> Result<(), Error> {
        #[cfg(unix)]
        fs::File::open(&self.directory)
            .and_then(|f| f.sync_all())
            .map_err(storage)?;
        Ok(())
    }
    fn stage_system(
        &mut self,
        accounts: &[SavedAccount],
        secrets: &Secrets,
    ) -> Result<Mode, Error> {
        self.cleanup()?;
        let id = identifier();
        // Journal the new entry before touching the OS store: startup can remove
        // it after a crash before the registry switches to the replacement.
        let mut staged = self.state.clone();
        staged.cleanup.push(id.clone());
        self.write(&staged)?;
        self.state = staged;
        self.sync_directory()?;
        let mut key = Zeroizing::new([0u8; 32]);
        OsRng.fill_bytes(key.as_mut());
        self.backend.set(
            &id,
            &Zeroizing::new(serde_json::to_string(&*key).map_err(storage)?),
        )?;
        let Mode::Password {
            nonce, ciphertext, ..
        } = seal(secrets, accounts, &key, [0; 16])?
        else {
            unreachable!()
        };
        Ok(Mode::System {
            entry: Some(id),
            nonce,
            ciphertext,
        })
    }

    pub fn commit(&mut self, accounts: Vec<SavedAccount>, secrets: Secrets) -> Result<(), Error> {
        if self.locked() {
            return Err(invalid("Unlock credentials first"));
        }
        self.cleanup()?;
        let (mode, next_key) = match &self.state.mode {
            Mode::Password { salt, .. } => {
                let key = self
                    .key
                    .as_ref()
                    .ok_or_else(|| invalid("Unlock credentials first"))?;
                (
                    seal(&secrets, &accounts, key, *salt)?,
                    Some(Zeroizing::new(**key)),
                )
            }
            Mode::System { .. } => (self.stage_system(&accounts, &secrets)?, None),
        };
        self.persist(accounts, secrets, mode, next_key)
    }
    pub fn protect(&mut self, password: &str) -> Result<(), Error> {
        if password.len() < 12 || password.len() > 1024 {
            return Err(invalid("Use a master password of 12 to 1024 bytes"));
        }
        let secrets = self
            .secrets
            .as_ref()
            .ok_or_else(|| invalid("Unlock credentials first"))?
            .clone();
        let mut salt = [0; 16];
        OsRng.fill_bytes(&mut salt);
        let key = key(password, &salt)?;
        self.cleanup()?;
        let mode = seal(&secrets, &self.state.accounts, &key, salt)?;
        self.persist(self.state.accounts.clone(), secrets, mode, Some(key))
    }
    pub fn unprotect(&mut self) -> Result<(), Error> {
        let secrets = self
            .secrets
            .as_ref()
            .ok_or_else(|| invalid("Unlock credentials first"))?
            .clone();
        let mode = self.stage_system(&self.state.accounts.clone(), &secrets)?;
        self.persist(self.state.accounts.clone(), secrets, mode, None)
    }
    pub fn unlock(&mut self, password: &str) -> Result<(), Error> {
        if password.len() > 1024 {
            return Err(invalid("Master password is too long"));
        }
        let Mode::Password {
            salt,
            nonce,
            ciphertext,
        } = &self.state.mode
        else {
            return Err(invalid("Password protection is not enabled"));
        };
        let key = key(password, salt)?;
        let aad = serde_json::to_vec(&self.state.accounts).map_err(storage)?;
        let plaintext = Zeroizing::new(
            XChaCha20Poly1305::new((&*key).into())
                .decrypt(
                    XNonce::from_slice(nonce),
                    Payload {
                        msg: ciphertext.as_slice(),
                        aad: &aad,
                    },
                )
                .map_err(|_| invalid("Incorrect master password or damaged credential vault"))?,
        );
        let secrets =
            serde_json::from_slice(&plaintext).map_err(|_| invalid("Damaged credential vault"))?;
        self.secrets = Some(secrets);
        self.key = Some(key);
        Ok(())
    }
    pub fn lock(&mut self) -> Result<(), Error> {
        if !self.protected() {
            return Err(invalid("Enable password protection before locking"));
        }
        self.secrets = None;
        self.key = None;
        Ok(())
    }
    pub fn reset(&mut self) -> Result<(), Error> {
        self.cleanup()?;
        let mut next = self.state.clone();
        for account in &mut next.accounts {
            account.configured = false;
        }
        if let Mode::System {
            entry: Some(id), ..
        } = &next.mode
        {
            next.cleanup.push(id.clone());
        }
        next.mode = Mode::System {
            entry: None,
            nonce: [0; 24],
            ciphertext: vec![],
        };
        self.write(&next)?;
        self.state = next;
        self.secrets = Some(BTreeMap::new());
        self.key = None;
        self.cleanup()
    }
}
