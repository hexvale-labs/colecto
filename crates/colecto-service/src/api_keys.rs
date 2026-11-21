//! Installation-local automation policy. Only verifiers are persisted.
use colecto_core::{Capability, Database, Row, Value};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Principal {
    Owner,
    ApiKey(String),
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
#[serde(deny_unknown_fields)]
pub struct KeyMetadata {
    pub id: String,
    pub name: String,
    pub accounts: Vec<String>,
    pub capabilities: Vec<Capability>,
    pub created_at: u64,
    pub expires_at: Option<u64>,
    pub last_used_at: Option<u64>,
    pub revoked_at: Option<u64>,
}

#[derive(Clone)]
struct StoredKey {
    metadata: KeyMetadata,
    verifier: [u8; 32],
}

const MAX_KEYS: usize = 128;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyInput {
    pub name: String,
    pub accounts: Vec<String>,
    pub capabilities: Vec<Capability>,
    pub expires_at: Option<u64>,
}

/// The keys are re-read from the database before every decision, so a key revoked by
/// another session never authenticates here.
pub struct ApiKeys {
    database: Database,
    keys: Vec<StoredKey>,
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn random() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| "Secure random source unavailable")?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

fn valid_metadata(key: &KeyMetadata) -> bool {
    key.id.len() == 64
        && key.id.bytes().all(|b| b.is_ascii_hexdigit())
        && !key.name.trim().is_empty()
        && key.name.len() <= 80
        && !key.name.chars().any(char::is_control)
        && !key.accounts.is_empty()
        && key.accounts.len() <= 64
        && key.accounts.iter().all(|a| !a.is_empty() && a.len() <= 128)
        && !key.capabilities.is_empty()
        && key.capabilities.len() <= 16
        && key.expires_at.is_none_or(|t| t > key.created_at)
}

fn invalid<E>(_: E) -> String {
    "Invalid API key policy".into()
}

fn json_column<T: serde::de::DeserializeOwned>(row: &Row, index: usize) -> Result<T, String> {
    serde_json::from_str(&row.text(index).map_err(invalid)?).map_err(invalid)
}

fn time_column(row: &Row, index: usize) -> Result<Option<u64>, String> {
    row.optional_integer(index)
        .map_err(invalid)?
        .map(|t| u64::try_from(t).map_err(invalid))
        .transpose()
}

fn read_key(row: &Row) -> Result<StoredKey, String> {
    Ok(StoredKey {
        metadata: KeyMetadata {
            id: row.text(0).map_err(invalid)?,
            name: row.text(1).map_err(invalid)?,
            accounts: json_column(row, 2)?,
            capabilities: json_column(row, 3)?,
            created_at: time_column(row, 4)?.ok_or_else(|| invalid(()))?,
            expires_at: time_column(row, 5)?,
            last_used_at: time_column(row, 6)?,
            revoked_at: time_column(row, 7)?,
        },
        verifier: row.blob(8).map_err(invalid)?.try_into().map_err(invalid)?,
    })
}

fn json(value: &impl Serialize) -> Result<String, String> {
    serde_json::to_string(value).map_err(|e| e.to_string())
}

fn to_db(time: u64) -> Result<i64, String> {
    i64::try_from(time).map_err(|_| "Invalid time".into())
}

impl ApiKeys {
    pub fn open(database: Database) -> Result<Self, String> {
        let mut keys = Self {
            database,
            keys: Vec::new(),
        };
        keys.reload()?;
        Ok(keys)
    }

    fn reload(&mut self) -> Result<(), String> {
        self.keys.clear();
        let keys = self
            .database
            .query(
                "SELECT id,name,accounts,capabilities,created_at,expires_at,last_used_at,revoked_at,verifier FROM api_keys ORDER BY rowid",
                vec![],
            )
            .map_err(|e| e.to_string())?
            .iter()
            .map(read_key)
            .collect::<Result<Vec<_>, _>>()?;
        if keys.len() > MAX_KEYS || keys.iter().any(|k| !valid_metadata(&k.metadata)) {
            return Err("Invalid API key policy".into());
        }
        self.keys = keys;
        Ok(())
    }

    /// Write one change, then re-read: the database stays authoritative either way.
    fn commit(&mut self, sql: &str, params: Vec<Value>) -> Result<(), String> {
        let result = self
            .database
            .write(vec![(sql, params)])
            .map_err(|_| "Could not persist API key policy".to_string());
        self.reload()?;
        result
    }

    pub fn list(&self) -> Vec<KeyMetadata> {
        self.keys.iter().map(|k| k.metadata.clone()).collect()
    }

    pub fn mint(&mut self, input: KeyInput, current: u64) -> Result<(KeyMetadata, String), String> {
        self.reload()?;
        let metadata = KeyMetadata {
            id: random()?,
            name: input.name.trim().into(),
            accounts: input.accounts,
            capabilities: input.capabilities,
            created_at: current,
            expires_at: input.expires_at,
            last_used_at: None,
            revoked_at: None,
        };
        if !valid_metadata(&metadata) || self.keys.len() >= MAX_KEYS {
            return Err(
                "Use a name of 1–80 bytes, explicit accounts and capabilities, and a future expiry; at most 128 keys are retained"
                    .into(),
            );
        }
        let secret = format!("colecto_{}.{}", metadata.id, random()?);
        let verifier: [u8; 32] = Sha256::digest(secret.as_bytes()).into();
        self.commit(
            "INSERT INTO api_keys(id,name,accounts,capabilities,created_at,expires_at,verifier) VALUES (?1,?2,?3,?4,?5,?6,?7)",
            vec![
                metadata.id.as_str().into(),
                metadata.name.as_str().into(),
                json(&metadata.accounts)?.into(),
                json(&metadata.capabilities)?.into(),
                to_db(metadata.created_at)?.into(),
                metadata.expires_at.map(to_db).transpose()?.into(),
                verifier.to_vec().into(),
            ],
        )?;
        Ok((metadata, secret))
    }

    pub fn authenticate(&mut self, secret: &str, current: u64) -> Result<Principal, String> {
        self.reload()?;
        let id = secret
            .strip_prefix("colecto_")
            .and_then(|s| s.split_once('.'))
            .map(|(id, _)| id)
            .ok_or("Invalid API key")?;
        let key = self
            .keys
            .iter()
            .find(|k| k.metadata.id == id)
            .ok_or("Invalid API key")?;
        let digest: [u8; 32] = Sha256::digest(secret.as_bytes()).into();
        if digest.ct_eq(&key.verifier).unwrap_u8() != 1
            || key.metadata.revoked_at.is_some()
            || key.metadata.expires_at.is_some_and(|t| t <= current)
        {
            return Err("Invalid API key".into());
        }
        // Persist at most once per minute, while checking expiry and revocation on every call.
        if key
            .metadata
            .last_used_at
            .is_none_or(|t| current.saturating_sub(t) >= 60)
        {
            self.commit(
                "UPDATE api_keys SET last_used_at=?2 WHERE id=?1",
                vec![id.into(), to_db(current)?.into()],
            )?;
        }
        Ok(Principal::ApiKey(id.into()))
    }

    pub fn allows(
        &mut self,
        id: &str,
        account: &str,
        capability: Capability,
        current: u64,
    ) -> bool {
        if self.reload().is_err() {
            return false;
        }
        self.keys.iter().any(|k| {
            k.metadata.id == id
                && k.metadata.revoked_at.is_none()
                && k.metadata.expires_at.is_none_or(|t| t > current)
                && k.metadata.accounts.iter().any(|a| a == account)
                && k.metadata.capabilities.contains(&capability)
        })
    }

    pub fn revoke(&mut self, id: &str, current: u64) -> Result<(), String> {
        self.reload()?;
        if !self.keys.iter().any(|k| k.metadata.id == id) {
            return Err("API key not found".into());
        }
        self.commit(
            "UPDATE api_keys SET revoked_at=?2 WHERE id=?1",
            vec![id.into(), to_db(current)?.into()],
        )
    }
}
