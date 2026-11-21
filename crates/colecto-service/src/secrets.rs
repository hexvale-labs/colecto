use colecto_core::{Accounts, Error, MailStore, SecretStore};
use std::{path::Path, sync::Arc};

pub fn open_accounts(directory: &Path) -> Result<Accounts, Box<dyn std::error::Error>> {
    let store = MailStore::open(directory)?;
    let namespace = format!("Colecto:{}", std::fs::canonicalize(directory)?.display());
    Ok(Accounts::open(store, Arc::new(SystemSecrets(namespace)))?)
}

pub fn open_accounts_with_secrets(
    directory: &Path,
    secrets: Arc<dyn SecretStore>,
) -> Result<Accounts, Box<dyn std::error::Error>> {
    let store = MailStore::open(directory)?;
    Ok(Accounts::open(store, secrets)?)
}

pub struct UnavailableSecrets;
impl SecretStore for UnavailableSecrets {
    fn get(&self, _: &str) -> Result<String, Error> {
        Err(Error::Invalid("System credential store unavailable".into()))
    }
    fn set(&self, _: &str, _: &str) -> Result<(), Error> {
        Err(Error::Invalid("System credential store unavailable".into()))
    }
    fn delete(&self, _: &str) -> Result<(), Error> {
        Err(Error::Invalid("System credential store unavailable".into()))
    }
}

struct SystemSecrets(String);

fn credential_error(_: keyring::Error) -> Error {
    Error::Invalid(
        "System credential store unavailable or locked. Unlock your desktop keychain, or enable master password protection before adding accounts."
            .into(),
    )
}

impl SystemSecrets {
    fn entry(&self, id: &str) -> Result<keyring::Entry, Error> {
        if !cfg!(any(
            target_os = "linux",
            target_os = "macos",
            target_os = "ios",
            target_os = "windows",
            target_os = "freebsd",
            target_os = "openbsd"
        )) {
            return Err(Error::Invalid(
                "System credential storage is not supported on this platform; use master password protection"
                    .into(),
            ));
        }
        keyring::Entry::new(&self.0, id).map_err(credential_error)
    }
}

impl SecretStore for SystemSecrets {
    fn get(&self, id: &str) -> Result<String, Error> {
        self.entry(id)?.get_password().map_err(credential_error)
    }

    fn set(&self, id: &str, value: &str) -> Result<(), Error> {
        self.entry(id)?
            .set_password(value)
            .map_err(credential_error)
    }

    fn delete(&self, id: &str) -> Result<(), Error> {
        match self.entry(id)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(credential_error(e)),
        }
    }
}
