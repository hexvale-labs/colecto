//! Only immutable bundled packages may execute in the trusted application origin.
use colecto_core::Database;
use colecto_extension_api::{API_VERSION, Capability, ExtensionManifest, ExtensionState};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, path::Path};

fn parse_manifest(source: &str) -> Result<ExtensionManifest, String> {
    let manifest: ExtensionManifest = toml::from_str(source).map_err(|e| e.to_string())?;
    if manifest.api_version != API_VERSION {
        return Err("Unsupported extension API version".into());
    }
    if manifest.id.is_empty()
        || !manifest
            .id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err("Invalid extension identifier".into());
    }
    if [&manifest.name, &manifest.description, &manifest.author]
        .iter()
        .any(|s| s.trim().is_empty())
    {
        return Err("Extension metadata cannot be empty".into());
    }
    if ![
        "shell", "mailbox", "reader", "composer", "accounts", "panel",
    ]
    .contains(&manifest.slot.as_str())
    {
        return Err("Unknown extension slot".into());
    }
    if manifest.capabilities.contains(&Capability::AppNavigate) {
        return Err("app.navigate is reserved for API keys".into());
    }
    for (index, capability) in manifest.capabilities.iter().enumerate() {
        if manifest.capabilities[..index].contains(capability) {
            return Err("Duplicate capability".into());
        }
    }
    Ok(manifest)
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Package {
    manifest: ExtensionManifest,
    module: Vec<u8>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Grant {
    id: String,
    account: String,
    capabilities: Vec<Capability>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    version: u32,
    disabled: HashSet<String>,
    grants: Vec<Grant>,
    packages: Vec<Package>,
}
#[derive(Clone)]
pub struct Extensions {
    manifests: Vec<ExtensionManifest>,
    policy: Policy,
    database: Option<Database>,
    epoch: u64,
}
const MAX_FILE: u64 = 4 * 1024 * 1024;
impl Extensions {
    pub fn bundled() -> Self {
        let manifests: Vec<ExtensionManifest> = [
            include_str!("../../../ui/src/extensions/mail-shell/extension.toml"),
            include_str!("../../../ui/src/extensions/inbox/extension.toml"),
            include_str!("../../../ui/src/extensions/reader/extension.toml"),
            include_str!("../../../ui/src/extensions/composer/extension.toml"),
            include_str!("../../../ui/src/extensions/accounts/extension.toml"),
        ]
        .into_iter()
        .map(|source| parse_manifest(source).expect("valid bundled manifest"))
        .collect();
        Self {
            manifests,
            policy: Policy {
                version: 1,
                disabled: HashSet::new(),
                grants: vec![],
                packages: vec![],
            },
            database: None,
            epoch: 0,
        }
    }
    pub fn open(database: Database, directory: &Path) -> Result<Self, String> {
        let mut extensions = Self::bundled();
        let empty = extensions.policy.clone();
        extensions.policy = crate::store::open(
            &database,
            directory,
            "extensions",
            Some(crate::store::Legacy {
                max_bytes: MAX_FILE,
                private: false,
            }),
            || empty,
            |bytes| {
                let policy: Policy = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
                extensions.check(&policy)?;
                Ok(policy)
            },
        )?;
        extensions.database = Some(database);
        Ok(extensions)
    }
    fn check(&self, policy: &Policy) -> Result<(), String> {
        if policy.version != 1 || policy.packages.len() > 16 {
            return Err("Unsupported extension policy".into());
        }
        let mut ids: HashSet<String> = self.manifests.iter().map(|m| m.id.clone()).collect();
        for package in &policy.packages {
            validate_package(&package.manifest, &package.module)?;
            if !ids.insert(package.manifest.id.clone()) {
                return Err("Duplicate extension ID".into());
            }
        }
        let candidate = Self {
            policy: policy.clone(),
            ..self.clone()
        };
        let mut keys = HashSet::new();
        for grant in &policy.grants {
            if !keys.insert((&grant.id, &grant.account))
                || !candidate.valid_grants(&grant.id, &grant.capabilities)
            {
                return Err("Invalid extension grants".into());
            }
        }
        if policy.disabled.iter().any(|id| !ids.contains(id)) {
            return Err("Unknown disabled extension".into());
        }
        Ok(())
    }
    fn manifest(&self, id: &str) -> Option<&ExtensionManifest> {
        self.manifests
            .iter()
            .chain(self.policy.packages.iter().map(|p| &p.manifest))
            .find(|m| m.id == id)
    }
    fn valid_grants(&self, id: &str, grants: &[Capability]) -> bool {
        self.manifest(id).is_some_and(|m| {
            grants
                .iter()
                .enumerate()
                .all(|(i, c)| m.capabilities.contains(c) && !grants[..i].contains(c))
        })
    }
    fn commit(&mut self, next: Self) -> Result<(), String> {
        if let Some(database) = &self.database {
            if serde_json::to_vec(&next.policy)
                .map_err(|e| e.to_string())?
                .len() as u64
                > MAX_FILE
            {
                return Err("Extension policy too large".into());
            }
            crate::store::save(database, "extensions", &next.policy)?;
        }
        *self = next;
        Ok(())
    }
    pub fn list(&self, account: &str) -> Vec<ExtensionState> {
        self.manifests
            .iter()
            .chain(self.policy.packages.iter().map(|p| &p.manifest))
            .map(|manifest| {
                let bundled = self.manifests.iter().any(|m| m.id == manifest.id);
                ExtensionState {
                    manifest: manifest.clone(),
                    bundled,
                    enabled: !self.policy.disabled.contains(&manifest.id),
                    grants: self
                        .policy
                        .grants
                        .iter()
                        .find(|g| g.id == manifest.id && g.account == account)
                        .map(|g| g.capabilities.clone())
                        .unwrap_or_else(|| {
                            if bundled {
                                manifest.capabilities.clone()
                            } else {
                                vec![]
                            }
                        }),
                }
            })
            .collect()
    }
    pub fn update(
        &mut self,
        id: &str,
        account: &str,
        enabled: bool,
        grants: Vec<Capability>,
    ) -> Result<(), String> {
        if !self.valid_grants(id, &grants) {
            return Err("Capabilities not declared by extension".into());
        }
        let mut next = self.clone();
        next.policy
            .grants
            .retain(|g| !(g.id == id && g.account == account));
        next.policy.grants.push(Grant {
            id: id.into(),
            account: account.into(),
            capabilities: grants,
        });
        if enabled {
            next.policy.disabled.remove(id);
        } else {
            next.policy.disabled.insert(id.into());
            next.epoch = next
                .epoch
                .checked_add(1)
                .ok_or("Extension generation exhausted")?;
        }
        self.commit(next)
    }
    pub fn install_package(&mut self, package: &[u8]) -> Result<(), String> {
        let (manifest, module) = crate::package::read(package)?;
        self.install(&manifest, module)
    }
    pub fn install(&mut self, source: &str, module: Vec<u8>) -> Result<(), String> {
        if source.len() > 8192 {
            return Err("Manifest too large".into());
        }
        let manifest = parse_manifest(source)?;
        validate_package(&manifest, &module)?;
        if self.manifest(&manifest.id).is_some() {
            return Err("Extension ID already installed".into());
        }
        if self.policy.packages.len() >= 16 {
            return Err("Extension package limit reached".into());
        }
        let mut next = self.clone();
        next.policy.disabled.insert(manifest.id.clone());
        next.policy.packages.push(Package { manifest, module });
        next.epoch = next
            .epoch
            .checked_add(1)
            .ok_or("Extension generation exhausted")?;
        self.commit(next)
    }
    pub fn remove(&mut self, id: &str) -> Result<(), String> {
        if !self.policy.packages.iter().any(|p| p.manifest.id == id) {
            return Err("Only installed third-party packages can be removed".into());
        }
        let mut next = self.clone();
        next.policy.packages.retain(|p| p.manifest.id != id);
        next.policy.grants.retain(|g| g.id != id);
        next.policy.disabled.remove(id);
        next.epoch = next
            .epoch
            .checked_add(1)
            .ok_or("Extension generation exhausted")?;
        self.commit(next)
    }
    pub fn module(&self, id: &str) -> Option<(u64, Vec<u8>)> {
        if self.policy.disabled.contains(id) {
            return None;
        }
        self.policy
            .packages
            .iter()
            .find(|p| p.manifest.id == id)
            .map(|p| (self.epoch, p.module.clone()))
    }
    pub fn allows(&self, id: &str, account: &str, capability: Capability) -> bool {
        self.list(account).iter().any(|state| {
            state.manifest.id == id && state.enabled && state.grants.contains(&capability)
        })
    }
    pub fn allows_guest(
        &self,
        id: &str,
        account: &str,
        capability: Capability,
        epoch: u64,
    ) -> bool {
        self.epoch == epoch
            && self.policy.packages.iter().any(|p| p.manifest.id == id)
            && self.allows(id, account, capability)
    }
    pub fn bundled_identity(&self, id: &str) -> bool {
        self.manifests.iter().any(|m| m.id == id)
    }
}
fn validate_package(manifest: &ExtensionManifest, module: &[u8]) -> Result<(), String> {
    parse_manifest(&toml::to_string(manifest).map_err(|e| e.to_string())?)?;
    // Only declarative panels are executable third-party UI in this ABI.
    if manifest.slot != "panel" {
        return Err("Wasm extensions require slot = panel".into());
    }
    super::sandbox::validate(module)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn toml_manifest_requires_metadata_and_known_permissions() {
        let source = include_str!("../../../ui/src/extensions/reader/extension.toml");
        assert_eq!(parse_manifest(source).unwrap().author, "Colecto");
        for invalid in [
            source.replace("api_version = 1", "api_version = 999"),
            source.replace("messages.read", "accounts.manage"),
            source.replace("messages.read", "app.navigate"),
            source.replace("author = \"Colecto\"", "author = \"\""),
            format!("{source}entrypoint = \"https://evil.test\"\n"),
            source.replace("id = \"reader\"", "id = \"../reader\""),
        ] {
            assert!(parse_manifest(&invalid).is_err(), "{invalid}");
        }
    }
    #[test]
    fn grants_cannot_escalate_and_revocation_is_account_scoped() {
        let mut extensions = Extensions::bundled();
        assert!(extensions.allows("composer", "a", Capability::MessagesSend));
        assert!(
            extensions
                .update("reader", "a", true, vec![Capability::MessagesSend])
                .is_err()
        );
        assert!(extensions.update("unknown", "a", true, vec![]).is_err());
        assert!(
            extensions
                .update("composer", "a", true, vec![Capability::DraftsRead])
                .is_ok()
        );
        assert!(!extensions.allows("composer", "a", Capability::MessagesSend));
        assert!(extensions.allows("composer", "b", Capability::MessagesSend));
        assert!(extensions.update("composer", "a", false, vec![]).is_ok());
        assert!(!extensions.allows("composer", "b", Capability::MessagesSend));
        assert!(!extensions.allows("host", "a", Capability::MessagesRead));
    }
    fn panel() -> String {
        include_str!("../../../ui/src/extensions/reader/extension.toml")
            .replace("reader", "example")
            .replace("slot = \"example\"", "slot = \"panel\"")
    }
    #[test]
    fn packages_and_revoked_grants_survive_restart_without_implicit_access() {
        let dir = tempfile::tempdir().unwrap();
        let module = wat::parse_str("(module (func (export \"run\")))").unwrap();
        let database = Database::open(dir.path()).unwrap();
        let open = || Extensions::open(database.clone(), dir.path()).unwrap();
        let mut extensions = open();
        extensions.install(&panel(), module.clone()).unwrap();
        assert!(extensions.module("example").is_none());
        assert!(!extensions.allows("example", "a", Capability::MessagesRead));
        extensions
            .update("example", "a", true, vec![Capability::MessagesRead])
            .unwrap();
        extensions.update("composer", "a", true, vec![]).unwrap();
        drop(extensions);
        let mut extensions = open();
        assert!(extensions.module("example").is_some());
        assert!(extensions.allows("example", "a", Capability::MessagesRead));
        assert!(!extensions.allows("example", "b", Capability::MessagesRead));
        assert!(!extensions.allows("composer", "a", Capability::MessagesSend));
        assert!(extensions.install(&panel(), module).is_err());
        let old_epoch = extensions.module("example").unwrap().0;
        extensions.remove("example").unwrap();
        extensions
            .install(
                &panel(),
                wat::parse_str("(module (func (export \"run\")))").unwrap(),
            )
            .unwrap();
        extensions
            .update("example", "a", true, vec![Capability::MessagesRead])
            .unwrap();
        assert!(!extensions.allows_guest("example", "a", Capability::MessagesRead, old_epoch));
        assert!(extensions.allows_guest(
            "example",
            "a",
            Capability::MessagesRead,
            extensions.module("example").unwrap().0
        ));
        extensions.remove("example").unwrap();
        assert!(!open().list("a").iter().any(|s| s.manifest.id == "example"));
    }
    #[test]
    fn corrupt_legacy_policy_fails_closed_and_stays_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::open(dir.path()).unwrap();
        std::fs::write(dir.path().join("extensions.json"), "corrupt").unwrap();
        assert!(Extensions::open(database.clone(), dir.path()).is_err());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("extensions.json")).unwrap(),
            "corrupt"
        );
        std::fs::write(
            dir.path().join("extensions.json"),
            r#"{"version":1,"disabled":["reader"],"grants":[],"packages":[]}"#,
        )
        .unwrap();
        let extensions = Extensions::open(database, dir.path()).unwrap();
        assert!(!extensions.allows("reader", "a", Capability::MessagesRead));
    }
}
