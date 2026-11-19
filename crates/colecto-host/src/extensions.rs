//! Extension policy and installed packages. Bundled manifests are compiled in; third-party
//! packages live in the Host database. See docs/extensions/ui-model.md.
use crate::package::MAX_INSTALLED;
use crate::package::manifest::{explain, parse as parse_manifest, validate as validate_manifest};
use colecto_core::Database;
use colecto_extension_api::{
    ApiSupport, Capability, ExtensionManifest, ExtensionState, api_support,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Package {
    manifest: ExtensionManifest,
}
/// Version 2 is the client-side model. Version 1 held packages of the old panel and overlay
/// system; they are dropped on startup (there is no migration).
const POLICY_VERSION: u32 = 2;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    version: u32,
    disabled: HashSet<String>,
    packages: Vec<Package>,
}
/// What an old policy may hold: only the IDs matter, to drop their packages and settings.
#[derive(Deserialize)]
struct OldPolicy {
    #[serde(default)]
    disabled: HashSet<String>,
    #[serde(default)]
    packages: Vec<serde_json::Value>,
}
struct CachedPackage {
    service: Option<Arc<crate::sandbox::Compiled>>,
    client: Arc<crate::package::Files>,
    assets: Arc<crate::package::Assets>,
}
#[derive(Clone)]
pub struct Extensions {
    setting_values: HashMap<String, serde_json::Map<String, serde_json::Value>>,
    /// Each extension's own small key-value store (`storage.get`/`storage.set`).
    storage: HashMap<String, serde_json::Map<String, serde_json::Value>>,
    manifests: Vec<ExtensionManifest>,
    policy: Policy,
    database: Option<Database>,
    epoch: u64,
    /// The generation of each enabled extension; credentials and service invocations are
    /// bound to it, and it changes whenever the extension is disabled, removed or reinstalled.
    generations: HashMap<String, u64>,
    cache: HashMap<String, Arc<CachedPackage>>,
    /// Service parts of bundled extensions, built with the UI and loaded from its assets.
    bundled_services: HashMap<String, Arc<crate::sandbox::Compiled>>,
    memory_packages: HashMap<String, Arc<Vec<u8>>>,
    /// `api_support`, replaceable in tests because no real version is discontinued yet.
    support: fn(u32) -> ApiSupport,
}
const MAX_POLICY_BYTES: u64 = 4 * 1024 * 1024;
/// Everything one extension stores, serialized.
pub const MAX_STORAGE_BYTES: usize = 64 * 1024;
impl Extensions {
    pub fn bundled() -> Self {
        let manifests: Vec<ExtensionManifest> = [
            include_str!("../../../ui/src/extensions/inbox/extension.toml"),
            include_str!("../../../ui/src/extensions/reader/extension.toml"),
            include_str!("../../../ui/src/extensions/composer/extension.toml"),
            include_str!("../../../ui/src/extensions/notifications/extension.toml"),
            include_str!("../../../ui/src/extensions/ai/extension.toml"),
        ]
        .into_iter()
        .map(|source| parse_manifest(source).expect("valid bundled manifest"))
        .collect();
        let mut extensions = Self {
            setting_values: HashMap::new(),
            storage: HashMap::new(),
            manifests,
            policy: Policy {
                version: POLICY_VERSION,
                disabled: HashSet::new(),
                packages: vec![],
            },
            database: None,
            epoch: 0,
            generations: HashMap::new(),
            cache: HashMap::new(),
            bundled_services: HashMap::new(),
            memory_packages: HashMap::new(),
            support: api_support,
        };
        let ids: Vec<String> = extensions.manifests.iter().map(|m| m.id.clone()).collect();
        for id in ids {
            extensions.bump(&id).expect("fresh generation");
        }
        extensions
    }
    pub fn open(database: Database) -> Result<Self, String> {
        Self::open_with(database, api_support)
    }
    fn open_with(database: Database, support: fn(u32) -> ApiSupport) -> Result<Self, String> {
        let mut extensions = Self::bundled();
        extensions.support = support;
        let empty = extensions.policy.clone();
        let old_packages = std::cell::RefCell::new(Vec::<String>::new());
        extensions.policy = crate::store::open(
            &database,
            "extensions",
            || empty.clone(),
            |bytes| {
                let mut policy = match serde_json::from_slice::<Policy>(bytes) {
                    Ok(policy) if policy.version == POLICY_VERSION => policy,
                    _ => {
                        // Packages from the old extension system: forget them, keep the rest.
                        let old: OldPolicy =
                            serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
                        *old_packages.borrow_mut() = old
                            .packages
                            .iter()
                            .filter_map(|p| p["manifest"]["id"].as_str().map(String::from))
                            .collect();
                        Policy {
                            disabled: old.disabled,
                            ..empty.clone()
                        }
                    }
                };
                // Bundled extensions that became core UI (`mail-shell`, `accounts`) or were
                // dropped may still be listed as disabled; forget them.
                let known: HashSet<String> = extensions
                    .manifests
                    .iter()
                    .map(|m| m.id.clone())
                    .chain(policy.packages.iter().map(|p| p.manifest.id.clone()))
                    .collect();
                policy.disabled.retain(|id| known.contains(id));
                extensions.check(&policy)?;
                Ok(policy)
            },
        )?;
        extensions.setting_values =
            crate::store::open(&database, "extension-settings", HashMap::new, |b| {
                serde_json::from_slice(b).map_err(|e| e.to_string())
            })?;
        extensions.storage =
            crate::store::open(&database, "extension-storage", HashMap::new, |b| {
                serde_json::from_slice(b).map_err(|e| e.to_string())
            })?;
        extensions.database = Some(database);
        let mut dropped = old_packages.into_inner();
        // A package that no longer reads or matches its manifest is dropped too, rather than
        // keeping Colecto from starting.
        let ids: Vec<String> = extensions
            .policy
            .packages
            .iter()
            .map(|p| p.manifest.id.clone())
            .collect();
        for id in ids {
            let enabled = !extensions.policy.disabled.contains(&id)
                && extensions
                    .manifest(&id)
                    .is_some_and(|m| extensions.discontinued(m).is_none());
            if enabled && let Err(error) = extensions.load(&id) {
                tracing::warn!(extension = %id, %error, "Dropping an extension package that cannot load");
                extensions.policy.packages.retain(|p| p.manifest.id != id);
                extensions.policy.disabled.remove(&id);
                dropped.push(id);
            }
        }
        if !dropped.is_empty() {
            let database = extensions.database.clone().expect("database attached");
            for id in &dropped {
                tracing::warn!(extension = %id, "Removed an extension built for the old extension system");
                extensions.setting_values.remove(id);
                extensions.storage.remove(id);
                let _ = database.remove_record("extension-packages", id, 1);
            }
            crate::store::save(&database, "extensions", &extensions.policy)?;
            crate::store::save(&database, "extension-settings", &extensions.setting_values)?;
            crate::store::save(&database, "extension-storage", &extensions.storage)?;
        }
        Ok(extensions)
    }
    fn check(&self, policy: &Policy) -> Result<(), String> {
        if policy.version != POLICY_VERSION || policy.packages.len() > MAX_INSTALLED {
            return Err("Unsupported extension policy".into());
        }
        let mut ids: HashSet<String> = self.manifests.iter().map(|m| m.id.clone()).collect();
        for package in &policy.packages {
            validate_manifest(&package.manifest)?;
            if !ids.insert(package.manifest.id.clone()) {
                return Err("Duplicate extension ID".into());
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
    fn commit(&mut self, next: Self) -> Result<(), String> {
        if let Some(database) = &self.database {
            if serde_json::to_vec(&next.policy)
                .map_err(|e| e.to_string())?
                .len() as u64
                > MAX_POLICY_BYTES
            {
                return Err("Extension policy too large".into());
            }
            crate::store::save(database, "extension-settings", &next.setting_values)?;
            crate::store::save(database, "extension-storage", &next.storage)?;
            crate::store::save(database, "extensions", &next.policy)?;
        }
        *self = next;
        Ok(())
    }
    /// Give `id` a new generation, invalidating everything bound to the old one.
    fn bump(&mut self, id: &str) -> Result<u64, String> {
        self.epoch = self
            .epoch
            .checked_add(1)
            .ok_or("Extension generation exhausted")?;
        self.generations.insert(id.into(), self.epoch);
        Ok(self.epoch)
    }
    /// The current generation of an enabled extension, or `None` while it is off or gone.
    pub fn generation(&self, id: &str) -> Option<u64> {
        if self.policy.disabled.contains(id) {
            return None;
        }
        self.generations.get(id).copied()
    }
    pub fn settings(&self, id: &str) -> Result<serde_json::Value, String> {
        let manifest = self.manifest(id).ok_or("Unknown extension")?;
        let mut values = serde_json::Map::new();
        for setting in &manifest.settings {
            let value = self
                .setting_values
                .get(id)
                .and_then(|v| v.get(&setting.key))
                .filter(|v| setting.accepts(v))
                .unwrap_or(&setting.default);
            values.insert(setting.key.clone(), value.clone());
        }
        Ok(serde_json::json!({"schema": manifest.settings, "values": values}))
    }
    pub fn settings_guest(&self, id: &str, epoch: u64) -> Result<serde_json::Value, String> {
        if self.generation(id) != Some(epoch) {
            return Err("Extension disabled or stale".into());
        }
        Ok(self.settings(id)?["values"].clone())
    }
    /// Whether every value fits the extension's settings schema, without saving anything.
    pub fn check_settings(
        &self,
        id: &str,
        values: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<(), String> {
        let manifest = self.manifest(id).ok_or("Unknown extension")?;
        for (key, value) in values {
            if !manifest
                .settings
                .iter()
                .any(|s| s.key == *key && s.accepts(value))
            {
                return Err(format!("Invalid setting: {key}"));
            }
        }
        Ok(())
    }
    pub fn update_settings(
        &mut self,
        id: &str,
        values: serde_json::Map<String, serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        self.check_settings(id, &values)?;
        let mut next = self.clone();
        next.setting_values
            .entry(id.into())
            .or_default()
            .extend(values);
        self.commit(next)?;
        self.settings(id)
    }
    /// One value from an extension's own store.
    pub fn stored(&self, id: &str, key: &str) -> serde_json::Value {
        self.storage
            .get(id)
            .and_then(|values| values.get(key))
            .cloned()
            .unwrap_or(serde_json::Value::Null)
    }
    /// Set (or, with `null`, delete) one value; the whole store stays within 64 KiB.
    pub fn store(&mut self, id: &str, key: &str, value: serde_json::Value) -> Result<(), String> {
        if self.manifest(id).is_none() {
            return Err("Unknown extension".into());
        }
        if key.is_empty()
            || key.len() > 64
            || !key
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_.-".contains(&b))
        {
            return Err("Storage keys are 1–64 bytes of a-z, 0-9, _, . and -".into());
        }
        let mut next = self.clone();
        let values = next.storage.entry(id.into()).or_default();
        if value.is_null() {
            values.remove(key);
        } else {
            values.insert(key.into(), value);
        }
        if serde_json::to_vec(values).map_err(|e| e.to_string())?.len() > MAX_STORAGE_BYTES {
            return Err("Extension storage is limited to 64 KiB".into());
        }
        self.commit(next)
    }
    pub fn list(&self, _account: &str) -> Vec<ExtensionState> {
        self.manifests
            .iter()
            .chain(self.policy.packages.iter().map(|p| &p.manifest))
            .map(|manifest| {
                let bundled = self.manifests.iter().any(|m| m.id == manifest.id);
                let discontinued = self.discontinued(manifest);
                ExtensionState {
                    manifest: manifest.clone(),
                    bundled,
                    enabled: discontinued.is_none() && !self.policy.disabled.contains(&manifest.id),
                    discontinued,
                }
            })
            .collect()
    }
    pub fn update(&mut self, id: &str, enabled: bool) -> Result<(), String> {
        let manifest = self.manifest(id).ok_or("Unknown extension")?;
        if enabled
            && let Some(error) = explain(manifest.api_version, (self.support)(manifest.api_version))
        {
            return Err(error);
        }
        let mut next = self.clone();
        if enabled {
            if !next.bundled_identity(id) && !next.cache.contains_key(id) {
                next.load(id)?;
            } else if !next.generations.contains_key(id) {
                next.bump(id)?;
            }
            next.policy.disabled.remove(id);
        } else {
            next.policy.disabled.insert(id.into());
            next.cache.remove(id);
            next.generations.remove(id);
        }
        self.commit(next)
    }
    fn load(&mut self, id: &str) -> Result<(), String> {
        let bytes = if let Some(database) = &self.database {
            database
                .get_record("extension-packages", id, 1)
                .map_err(|e| e.to_string())?
                .ok_or("Missing extension package")?
        } else {
            self.memory_packages
                .get(id)
                .ok_or("Missing extension package")?
                .as_ref()
                .clone()
        };
        let package = crate::package::read(&bytes)?;
        let manifest = parse_manifest(&package.manifest)?;
        if self.manifest(id).is_none_or(|expected| {
            serde_json::to_value(expected).ok() != serde_json::to_value(&manifest).ok()
        }) {
            return Err("Stored extension manifest mismatch".into());
        }
        crate::package::check(&manifest, &package)?;
        let cached = cache_entry(package)?;
        self.bump(id)?;
        self.cache.insert(id.into(), Arc::new(cached));
        Ok(())
    }
    pub fn install_package(&mut self, bytes: &[u8]) -> Result<(), String> {
        let package = crate::package::read(bytes)?;
        let manifest = parse_manifest(&package.manifest)?;
        validate_manifest(&manifest)?;
        crate::package::check(&manifest, &package)?;
        if self.manifest(&manifest.id).is_some() {
            return Err(format!(
                "An extension with the ID `{}` is already installed",
                manifest.id
            ));
        }
        if self.policy.packages.len() >= MAX_INSTALLED {
            return Err("Extension package limit reached".into());
        }
        let cached = cache_entry(package)?;
        let mut next = self.clone();
        let id = manifest.id.clone();
        if let Some(database) = &next.database {
            database
                .set_record("extension-packages", &id, 1, bytes)
                .map_err(|e| e.to_string())?;
        } else {
            next.memory_packages
                .insert(id.clone(), Arc::new(bytes.to_vec()));
        }
        next.bump(&id)?;
        next.cache.insert(id.clone(), Arc::new(cached));
        next.policy.packages.push(Package { manifest });
        self.commit(next)
    }
    #[cfg(test)]
    pub fn install(&mut self, package: &crate::package::Package) -> Result<(), String> {
        self.install_package(&crate::package::write(package))
    }
    pub fn remove(&mut self, id: &str) -> Result<(), String> {
        if !self.policy.packages.iter().any(|p| p.manifest.id == id) {
            return Err("Only installed third-party packages can be removed".into());
        }
        let mut next = self.clone();
        next.setting_values.remove(id);
        next.storage.remove(id);
        next.policy.packages.retain(|p| p.manifest.id != id);
        next.cache.remove(id);
        next.generations.remove(id);
        next.memory_packages.remove(id);
        next.policy.disabled.remove(id);
        self.commit(next)?;
        if let Some(database) = &self.database {
            database
                .remove_record("extension-packages", id, 1)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    pub fn assets(&self, id: &str) -> Option<Arc<crate::package::Assets>> {
        if self.policy.disabled.contains(id) {
            return None;
        }
        self.cache.get(id).map(|p| p.assets.clone())
    }
    /// The files under `client/` of an enabled third-party extension.
    pub fn client_files(&self, id: &str) -> Option<Arc<crate::package::Files>> {
        if self.policy.disabled.contains(id) {
            return None;
        }
        self.cache.get(id).map(|p| p.client.clone())
    }
    /// The compiled service part of an enabled extension, with its generation.
    pub fn service(&self, id: &str) -> Option<(u64, Arc<crate::sandbox::Compiled>)> {
        let generation = self.generation(id)?;
        let service = match self.bundled_services.get(id) {
            Some(service) => service.clone(),
            None => self.cache.get(id)?.service.clone()?,
        };
        Some((generation, service))
    }
    /// Bundled extensions whose manifest names a service part, with its file name.
    pub fn bundled_service_names(&self) -> Vec<(String, String)> {
        self.manifests
            .iter()
            .filter_map(|m| m.service.clone().map(|service| (m.id.clone(), service)))
            .collect()
    }
    pub fn load_bundled_service(&mut self, id: &str, bytes: &[u8]) -> Result<(), String> {
        self.bundled_services
            .insert(id.into(), crate::sandbox::compile(bytes)?);
        Ok(())
    }
    /// Enabled extensions whose service part hears about new mail (`messages.list`).
    pub fn mail_services(&self) -> Vec<(String, u64, Arc<crate::sandbox::Compiled>)> {
        self.list("")
            .into_iter()
            .filter(|s| s.enabled && s.manifest.capabilities.contains(&Capability::MessagesList))
            .filter_map(|s| {
                let (generation, service) = self.service(&s.manifest.id)?;
                Some((s.manifest.id, generation, service))
            })
            .collect()
    }
    pub fn has_client(&self, id: &str) -> bool {
        self.manifest(id).is_some_and(|m| m.client.is_some())
    }
    pub fn allows(&self, id: &str, account: &str, capability: Capability) -> bool {
        self.list(account).iter().any(|state| {
            state.manifest.id == id
                && state.enabled
                && state.manifest.capabilities.contains(&capability)
        })
    }
    /// [`Self::allows`] for a caller bound to a generation: a service invocation or a
    /// client part's credential.
    pub fn allows_generation(
        &self,
        id: &str,
        account: &str,
        capability: Capability,
        generation: u64,
    ) -> bool {
        self.generation(id) == Some(generation) && self.allows(id, account, capability)
    }
    /// The release that discontinued this manifest's API version, if any.
    fn discontinued(&self, manifest: &ExtensionManifest) -> Option<String> {
        match (self.support)(manifest.api_version) {
            ApiSupport::Discontinued(release) => Some(release.into()),
            _ => None,
        }
    }
    pub fn bundled_identity(&self, id: &str) -> bool {
        self.manifests.iter().any(|m| m.id == id)
    }
}
fn cache_entry(package: crate::package::Package) -> Result<CachedPackage, String> {
    Ok(CachedPackage {
        service: match &package.service {
            Some((_, bytes)) => Some(crate::sandbox::compile(bytes)?),
            None => None,
        },
        client: Arc::new(package.client),
        assets: Arc::new(package.assets),
    })
}
#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) const EXAMPLE: &str =
        include_str!("../../../docs/extensions/examples/hello-pane/manifest.toml");
    /// The example package with `manifest` (and, given one, a service module).
    pub(crate) fn package(manifest: &str, service: Option<Vec<u8>>) -> crate::package::Package {
        let mut package = crate::package::Package {
            manifest: if service.is_some() && !manifest.contains("service =") {
                manifest.replace("client = ", "service = \"service.wasm\"\nclient = ")
            } else {
                manifest.into()
            },
            ..Default::default()
        };
        package.client.insert(
            "client/main.js".into(),
            include_bytes!("../../../docs/extensions/examples/hello-pane/client/main.js").to_vec(),
        );
        package.service = service.map(|bytes| ("service.wasm".into(), bytes));
        package
    }
    #[test]
    fn duplicate_ids_and_bus_subscriptions() {
        let mut extensions = Extensions::bundled();
        for id in ["reader", "hello-pane"] {
            let source = EXAMPLE.replace("hello-pane", id);
            if id == "hello-pane" {
                extensions.install(&package(&source, None)).unwrap();
            }
            assert_eq!(
                extensions.install(&package(&source, None)).unwrap_err(),
                format!("An extension with the ID `{id}` is already installed")
            );
        }
        let bus = EXAMPLE.replace("account.read", "extensions.bus");
        let bus = bus.split("[pane]").next().unwrap();
        assert!(
            parse_manifest(&format!(
                "{bus}\n[bus]\nsubscribe = ['reader/v1/message_opened', 'composer/v0.1.2/x']"
            ))
            .is_ok()
        );
        let base = EXAMPLE.split("[pane]").next().unwrap();
        for source in [
            format!("{base}\n[bus]\nsubscribe = []"),
            format!("{bus}\n[bus]\nsubscribe = ['reader/INVALID']"),
            format!(
                "{bus}\n[bus]\nsubscribe = [{}]",
                vec!["'reader/v1/x'"; 33].join(",")
            ),
        ] {
            assert!(parse_manifest(&source).is_err());
        }
    }
    #[test]
    fn packages_must_hold_what_the_manifest_names() {
        let mut extensions = Extensions::bundled();
        let mut missing = package(EXAMPLE, None);
        missing.client.clear();
        missing
            .client
            .insert("client/other.js".into(), b"1".to_vec());
        assert!(extensions.install(&missing).is_err());
        let undeclared = crate::package::Package {
            service: Some(("service.wasm".into(), wat::parse_str("(module)").unwrap())),
            ..package(EXAMPLE, None)
        };
        assert!(extensions.install(&undeclared).is_err());
        // The service part must be valid Wasm.
        assert!(
            extensions
                .install(&package(EXAMPLE, Some(b"nope".to_vec())))
                .is_err()
        );
        assert!(
            extensions
                .install(&package(EXAMPLE, Some(wat::parse_str("(module)").unwrap())))
                .is_ok()
        );
        assert!(
            extensions
                .client_files("hello-pane")
                .unwrap()
                .contains_key("client/main.js")
        );
        assert!(extensions.service("hello-pane").is_some());
    }
    #[test]
    fn generations_change_on_disable_remove_and_reinstall() {
        let mut extensions = Extensions::bundled();
        let reader = extensions.generation("reader").unwrap();
        extensions.update("reader", false).unwrap();
        assert_eq!(extensions.generation("reader"), None);
        extensions.update("reader", true).unwrap();
        assert_ne!(extensions.generation("reader"), Some(reader));
        extensions.install(&package(EXAMPLE, None)).unwrap();
        let first = extensions.generation("hello-pane").unwrap();
        for account in ["a", "b"] {
            assert!(extensions.allows_generation(
                "hello-pane",
                account,
                Capability::AccountRead,
                first
            ));
            assert!(!extensions.allows("hello-pane", account, Capability::MessagesRead));
        }
        extensions.update("hello-pane", false).unwrap();
        assert!(!extensions.allows("hello-pane", "a", Capability::AccountRead));
        assert!(extensions.client_files("hello-pane").is_none());
        extensions.update("hello-pane", true).unwrap();
        let second = extensions.generation("hello-pane").unwrap();
        assert!(!extensions.allows_generation("hello-pane", "a", Capability::AccountRead, first));
        extensions.remove("hello-pane").unwrap();
        assert_eq!(extensions.generation("hello-pane"), None);
        extensions.install(&package(EXAMPLE, None)).unwrap();
        assert!(!extensions.allows_generation("hello-pane", "a", Capability::AccountRead, second));
    }
    #[test]
    fn discontinued_api_versions_stay_listed_but_never_run() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::open(dir.path()).unwrap();
        let mut extensions = Extensions::open(database.clone()).unwrap();
        extensions.install(&package(EXAMPLE, None)).unwrap();
        fn dropped(version: u32) -> ApiSupport {
            if version == 1 {
                ApiSupport::Discontinued("v9.0.0")
            } else {
                api_support(version)
            }
        }
        let mut upgraded = Extensions::open_with(database, dropped).unwrap();
        let state = upgraded
            .list("demo")
            .into_iter()
            .find(|s| s.manifest.id == "hello-pane")
            .unwrap();
        assert!(!state.enabled);
        assert_eq!(state.discontinued.as_deref(), Some("v9.0.0"));
        assert!(upgraded.client_files("hello-pane").is_none());
        assert!(
            upgraded
                .update("hello-pane", true)
                .unwrap_err()
                .contains("v9.0.0")
        );
        upgraded.remove("hello-pane").unwrap();
    }
    #[test]
    fn storage_is_per_extension_bounded_and_removed_with_it() {
        let dir = tempfile::tempdir().unwrap();
        let mut extensions = Extensions::open(Database::open(dir.path()).unwrap()).unwrap();
        extensions.install(&package(EXAMPLE, None)).unwrap();
        extensions
            .store("hello-pane", "seen", serde_json::json!(["1"]))
            .unwrap();
        assert_eq!(
            extensions.stored("hello-pane", "seen"),
            serde_json::json!(["1"])
        );
        assert_eq!(extensions.stored("reader", "seen"), serde_json::Value::Null);
        assert!(
            extensions
                .store("hello-pane", "Bad Key", serde_json::json!(1))
                .is_err()
        );
        let big = "x".repeat(MAX_STORAGE_BYTES);
        assert!(
            extensions
                .store("hello-pane", "big", serde_json::json!(big))
                .is_err()
        );
        assert_eq!(
            Extensions::open(extensions.database.clone().unwrap())
                .unwrap()
                .stored("hello-pane", "seen"),
            serde_json::json!(["1"])
        );
        extensions.remove("hello-pane").unwrap();
        extensions.install(&package(EXAMPLE, None)).unwrap();
        assert_eq!(
            extensions.stored("hello-pane", "seen"),
            serde_json::Value::Null
        );
    }
    #[test]
    fn installed_package_count_boundary() {
        let mut extensions = Extensions::bundled();
        for index in 0..MAX_INSTALLED {
            let source = EXAMPLE.replace("hello-pane", &format!("pane-{index}"));
            extensions.install(&package(&source, None)).unwrap();
        }
        let source = EXAMPLE.replace("hello-pane", "one-too-many");
        assert_eq!(
            extensions.install(&package(&source, None)).unwrap_err(),
            "Extension package limit reached"
        );
    }
    #[test]
    fn enabled_policy_and_packages_survive_restart() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::open(dir.path()).unwrap();
        let mut extensions = Extensions::open(database.clone()).unwrap();
        extensions.install(&package(EXAMPLE, None)).unwrap();
        extensions.update("reader", false).unwrap();
        let reopened = Extensions::open(database).unwrap();
        assert!(reopened.client_files("hello-pane").is_some());
        assert_eq!(reopened.generation("reader"), None);
        assert!(reopened.generation("hello-pane").is_some());
    }
    #[test]
    fn packages_from_the_old_extension_system_are_dropped_on_startup() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::open(dir.path()).unwrap();
        let old = serde_json::json!({
            "version": 1,
            "disabled": ["mail-shell", "accounts", "ai", "hello-pane"],
            "packages": [{"manifest": {"id": "hello-pane", "name": "Connection panel",
                "description": "d", "author": "a", "api_version": 1, "slot": "panel",
                "capabilities": ["account.read"]}}]
        });
        crate::store::save(&database, "extensions", &old).unwrap();
        crate::store::save(
            &database,
            "extension-settings",
            &serde_json::json!({"hello-pane": {"speed": 3}}),
        )
        .unwrap();
        database
            .set_record("extension-packages", "hello-pane", 1, b"old")
            .unwrap();
        let extensions = Extensions::open(database.clone()).unwrap();
        assert!(extensions.policy.packages.is_empty());
        assert_eq!(
            extensions.policy.disabled,
            HashSet::from(["ai".to_string()])
        );
        assert!(extensions.setting_values.is_empty());
        assert!(
            database
                .get_record("extension-packages", "hello-pane", 1)
                .unwrap()
                .is_none()
        );
        // The cleaned policy is saved, so the next start reads it as is.
        Extensions::open(database).unwrap();
    }
}
#[cfg(test)]
mod settings_tests {
    use super::*;
    #[test]
    fn settings_validate_persist_and_clear_on_removal() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::open(dir.path()).unwrap();
        let mut extensions = Extensions::open(database.clone()).unwrap();
        let source = format!(
            "{}\n[[settings]]\nkey='speed'\nlabel='Speed'\ntype='range'\ndefault=10\nmin=2\nmax=30\n",
            super::tests::EXAMPLE
        );
        extensions
            .install(&super::tests::package(&source, None))
            .unwrap();
        for value in [
            serde_json::json!({"speed":31}),
            serde_json::json!({"speed":"10"}),
            serde_json::json!({"unknown":10}),
        ] {
            assert!(
                extensions
                    .update_settings("hello-pane", value.as_object().unwrap().clone())
                    .is_err()
            );
        }
        extensions
            .update_settings(
                "hello-pane",
                serde_json::json!({"speed":12}).as_object().unwrap().clone(),
            )
            .unwrap();
        assert_eq!(
            Extensions::open(database.clone())
                .unwrap()
                .settings("hello-pane")
                .unwrap()["values"]["speed"],
            12
        );
        extensions.remove("hello-pane").unwrap();
        extensions
            .install(&super::tests::package(&source, None))
            .unwrap();
        assert_eq!(
            extensions.settings("hello-pane").unwrap()["values"]["speed"],
            10
        );
        for bad in [
            source.replace("default=10", "default=31"),
            source.replace("key='speed'", "key='Bad'"),
            source.replace("label='Speed'", "label='Speed'\nunknown=1"),
        ] {
            assert!(parse_manifest(&bad).is_err());
        }
    }
}
