//! Installation-local automation policy. Only verifiers are persisted.
use colecto_extension_api::Capability;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    io::Write,
    path::{Path, PathBuf},
};
use subtle::ConstantTimeEq;

#[derive(Clone, Debug)]
pub enum Principal {
    Owner,
    ApiKey(String),
}
#[derive(Clone, Serialize, Deserialize)]
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
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredKey {
    metadata: KeyMetadata,
    verifier: [u8; 32],
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    version: u32,
    keys: Vec<StoredKey>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyInput {
    pub name: String,
    pub accounts: Vec<String>,
    pub capabilities: Vec<Capability>,
    pub expires_at: Option<u64>,
}
pub struct ApiKeys {
    path: PathBuf,
    policy: Policy,
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
        && key.capabilities.len() <= 8
        && key.expires_at.is_none_or(|t| t > key.created_at)
}
impl ApiKeys {
    pub fn open(directory: &Path) -> Result<Self, String> {
        let path = directory.join("api-keys.json");
        let policy = match std::fs::symlink_metadata(&path) {
            Ok(meta) => {
                if !meta.is_file() || meta.len() > 2_000_000 {
                    return Err("Invalid API key policy".into());
                }
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if meta.permissions().mode() & 0o077 != 0 {
                        return Err("API key policy must be private".into());
                    }
                }
                let policy: Policy = serde_json::from_slice(
                    &std::fs::read(&path).map_err(|_| "Cannot read API key policy")?,
                )
                .map_err(|_| "Invalid API key policy")?;
                if policy.version != 1
                    || policy.keys.len() > 128
                    || policy.keys.iter().any(|k| !valid_metadata(&k.metadata))
                    || policy.keys.iter().enumerate().any(|(i, k)| {
                        policy.keys[..i]
                            .iter()
                            .any(|v| v.metadata.id == k.metadata.id)
                    })
                {
                    return Err("Invalid API key policy".into());
                }
                policy
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Policy {
                version: 1,
                keys: vec![],
            },
            Err(_) => return Err("Cannot read API key policy".into()),
        };
        Ok(Self { path, policy })
    }
    fn persist(&self, policy: &Policy) -> Result<(), String> {
        let temporary = self.path.with_extension(format!("{}.tmp", random()?));
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temporary)?;
            file.write_all(&serde_json::to_vec(policy)?)?;
            file.sync_all()?;
            std::fs::rename(&temporary, &self.path)?;
            #[cfg(unix)]
            std::fs::File::open(self.path.parent().unwrap())?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temporary);
        }
        result.map_err(|_| "Could not persist API key policy".into())
    }
    fn reload(&mut self) -> Result<(), String> {
        self.policy = Policy {
            version: 1,
            keys: vec![],
        };
        self.policy = Self::open(self.path.parent().ok_or("Invalid API key policy path")?)?.policy;
        Ok(())
    }
    fn commit(&mut self, policy: Policy) -> Result<(), String> {
        let result = self.persist(&policy);
        // A failed directory fsync can follow a successful rename. Disk remains authoritative.
        self.reload()?;
        result
    }
    pub fn list(&self) -> Vec<KeyMetadata> {
        self.policy
            .keys
            .iter()
            .map(|k| k.metadata.clone())
            .collect()
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
        if !valid_metadata(&metadata) || self.policy.keys.len() >= 128 {
            return Err("Use a name of 1–80 bytes, explicit accounts and capabilities, and a future expiry; at most 128 keys are retained".into());
        }
        let secret = format!("colecto_{}.{}", metadata.id, random()?);
        let mut policy = self.policy.clone();
        policy.keys.push(StoredKey {
            metadata: metadata.clone(),
            verifier: Sha256::digest(secret.as_bytes()).into(),
        });
        self.commit(policy)?;
        Ok((metadata, secret))
    }
    pub fn authenticate(&mut self, secret: &str, current: u64) -> Result<Principal, String> {
        self.reload()?;
        let id = secret
            .strip_prefix("colecto_")
            .and_then(|s| s.split_once('.'))
            .map(|(id, _)| id)
            .ok_or("Invalid API key")?;
        let index = self
            .policy
            .keys
            .iter()
            .position(|k| k.metadata.id == id)
            .ok_or("Invalid API key")?;
        let key = &self.policy.keys[index];
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
            let mut policy = self.policy.clone();
            policy.keys[index].metadata.last_used_at = Some(current);
            self.commit(policy)?;
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
        self.policy.keys.iter().any(|k| {
            k.metadata.id == id
                && k.metadata.revoked_at.is_none()
                && k.metadata.expires_at.is_none_or(|t| t > current)
                && k.metadata.accounts.iter().any(|a| a == account)
                && k.metadata.capabilities.contains(&capability)
        })
    }
    pub fn revoke(&mut self, id: &str, current: u64) -> Result<(), String> {
        self.reload()?;
        let mut policy = self.policy.clone();
        let key = policy
            .keys
            .iter_mut()
            .find(|k| k.metadata.id == id)
            .ok_or("API key not found")?;
        key.metadata.revoked_at = Some(current);
        self.commit(policy)?;
        Ok(())
    }
}
