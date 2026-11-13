//! Only immutable bundled packages may execute in the trusted application origin.
use crate::package::MAX_INSTALLED;
use crate::package::manifest::{parse as parse_manifest, validate as validate_manifest};
use colecto_core::Database;
use colecto_extension_api::{Capability, ExtensionManifest, ExtensionState};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Package {
    manifest: ExtensionManifest,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    module: Vec<u8>,
    #[serde(default, skip_serializing_if = "crate::package::Assets::is_empty")]
    assets: crate::package::Assets,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Policy {
    version: u32,
    disabled: HashSet<String>,
    #[serde(default, rename = "grants", skip_serializing)]
    _obsolete_grants: serde_json::Value,
    packages: Vec<Package>,
}
struct CachedPackage {
    generation: u64,
    module: Arc<crate::sandbox::Compiled>,
    assets: Arc<crate::package::Assets>,
}
#[derive(Clone)]
pub struct Extensions {
    setting_values: HashMap<String, serde_json::Map<String, serde_json::Value>>,
    manifests: Vec<ExtensionManifest>,
    policy: Policy,
    database: Option<Database>,
    epoch: u64,
    cache: HashMap<String, Arc<CachedPackage>>,
    memory_packages: HashMap<String, Arc<Vec<u8>>>,
}
const MAX_POLICY_BYTES: u64 = 4 * 1024 * 1024;
impl Extensions {
    pub fn bundled() -> Self {
        let manifests: Vec<ExtensionManifest> = [
            include_str!("../../../ui/src/extensions/mail-shell/extension.toml"),
            include_str!("../../../ui/src/extensions/inbox/extension.toml"),
            include_str!("../../../ui/src/extensions/reader/extension.toml"),
            include_str!("../../../ui/src/extensions/composer/extension.toml"),
            include_str!("../../../ui/src/extensions/accounts/extension.toml"),
            include_str!("../../../ui/src/extensions/notifications/extension.toml"),
        ]
        .into_iter()
        .map(|source| parse_manifest(source).expect("valid bundled manifest"))
        .collect();
        Self {
            setting_values: HashMap::new(),
            manifests,
            policy: Policy {
                version: 1,
                disabled: HashSet::new(),
                _obsolete_grants: serde_json::Value::Null,
                packages: vec![],
            },
            database: None,
            epoch: 0,
            cache: HashMap::new(),
            memory_packages: HashMap::new(),
        }
    }
    pub fn open(database: Database) -> Result<Self, String> {
        let mut extensions = Self::bundled();
        let empty = extensions.policy.clone();
        extensions.policy = crate::store::open(
            &database,
            "extensions",
            || empty,
            |bytes| {
                let policy: Policy = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
                extensions.check(&policy)?;
                Ok(policy)
            },
        )?;
        extensions.setting_values =
            crate::store::open(&database, "extension-settings", HashMap::new, |b| {
                serde_json::from_slice(b).map_err(|e| e.to_string())
            })?;
        extensions.database = Some(database);
        let mut legacy = false;
        for package in &mut extensions.policy.packages {
            if !package.module.is_empty() {
                let source = toml::to_string(&package.manifest).map_err(|e| e.to_string())?;
                let bytes =
                    crate::package::write_with_assets(&source, &package.module, &package.assets);
                extensions
                    .database
                    .as_ref()
                    .expect("database attached")
                    .set_record("extension-packages", &package.manifest.id, 1, &bytes)
                    .map_err(|e| e.to_string())?;
                package.module.clear();
                package.assets.clear();
                legacy = true;
            }
        }
        if legacy {
            crate::store::save(
                extensions.database.as_ref().expect("database attached"),
                "extensions",
                &extensions.policy,
            )?;
        }
        let enabled: Vec<_> = extensions
            .policy
            .packages
            .iter()
            .filter(|p| !extensions.policy.disabled.contains(&p.manifest.id))
            .map(|p| p.manifest.id.clone())
            .collect();
        for id in enabled {
            extensions.load(&id)?;
        }
        Ok(extensions)
    }
    fn check(&self, policy: &Policy) -> Result<(), String> {
        if policy.version != 1 || policy.packages.len() > MAX_INSTALLED {
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
            crate::store::save(database, "extensions", &next.policy)?;
        }
        *self = next;
        Ok(())
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
        if !self.cache.get(id).is_some_and(|p| p.generation == epoch)
            || self.policy.disabled.contains(id)
        {
            return Err("Extension disabled or stale".into());
        }
        Ok(self.settings(id)?["values"].clone())
    }
    pub fn update_settings(
        &mut self,
        id: &str,
        values: serde_json::Map<String, serde_json::Value>,
    ) -> Result<serde_json::Value, String> {
        let manifest = self.manifest(id).ok_or("Unknown extension")?;
        for (key, value) in &values {
            if !manifest
                .settings
                .iter()
                .any(|s| s.key == *key && s.accepts(value))
            {
                return Err(format!("Invalid setting: {key}"));
            }
        }
        let mut next = self.clone();
        next.setting_values
            .entry(id.into())
            .or_default()
            .extend(values);
        self.commit(next)?;
        self.settings(id)
    }
    pub fn list(&self, _account: &str) -> Vec<ExtensionState> {
        self.manifests
            .iter()
            .chain(self.policy.packages.iter().map(|p| &p.manifest))
            .map(|manifest| {
                let bundled = self.manifests.iter().any(|m| m.id == manifest.id);
                ExtensionState {
                    manifest: manifest.clone(),
                    bundled,
                    enabled: !self.policy.disabled.contains(&manifest.id),
                }
            })
            .collect()
    }
    pub fn update(&mut self, id: &str, enabled: bool) -> Result<(), String> {
        if self.manifest(id).is_none() {
            return Err("Unknown extension".into());
        }
        let mut next = self.clone();
        if enabled {
            if !next.bundled_identity(id) && !next.cache.contains_key(id) {
                next.load(id)?;
            }
            next.policy.disabled.remove(id);
        } else {
            next.policy.disabled.insert(id.into());
            next.cache.remove(id);
            next.epoch = next
                .epoch
                .checked_add(1)
                .ok_or("Extension generation exhausted")?;
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
        let (source, module, assets) = crate::package::read_with_assets(&bytes)?;
        let manifest = parse_manifest(&source)?;
        if self.manifest(id).is_none_or(|expected| {
            serde_json::to_value(expected).ok() != serde_json::to_value(&manifest).ok()
        }) {
            return Err("Stored extension manifest mismatch".into());
        }
        self.epoch = self
            .epoch
            .checked_add(1)
            .ok_or("Extension generation exhausted")?;
        self.cache.insert(
            id.into(),
            Arc::new(CachedPackage {
                generation: self.epoch,
                module: crate::sandbox::compile(&module)?,
                assets: Arc::new(assets),
            }),
        );
        Ok(())
    }
    pub fn install_package(&mut self, bytes: &[u8]) -> Result<(), String> {
        let (source, module, assets) = crate::package::read_with_assets(bytes)?;
        let manifest = parse_manifest(&source)?;
        validate_manifest(&manifest)?;
        if self.manifest(&manifest.id).is_some() {
            return Err(format!(
                "An extension with the ID `{}` is already installed",
                manifest.id
            ));
        }
        if self.policy.packages.len() >= MAX_INSTALLED {
            return Err("Extension package limit reached".into());
        }
        let compiled = crate::sandbox::compile(&module)?;
        let mut next = self.clone();
        next.epoch = next
            .epoch
            .checked_add(1)
            .ok_or("Extension generation exhausted")?;
        let id = manifest.id.clone();
        if let Some(database) = &next.database {
            database
                .set_record("extension-packages", &id, 1, bytes)
                .map_err(|e| e.to_string())?;
        } else {
            next.memory_packages
                .insert(id.clone(), Arc::new(bytes.to_vec()));
        }
        next.cache.insert(
            id.clone(),
            Arc::new(CachedPackage {
                generation: next.epoch,
                module: compiled,
                assets: Arc::new(assets),
            }),
        );
        next.policy.packages.push(Package {
            manifest,
            module: Vec::new(),
            assets: Default::default(),
        });
        self.commit(next)
    }
    #[cfg(test)]
    pub fn install(&mut self, source: &str, module: Vec<u8>) -> Result<(), String> {
        self.install_package(&crate::package::write(source, &module))
    }
    pub fn remove(&mut self, id: &str) -> Result<(), String> {
        if !self.policy.packages.iter().any(|p| p.manifest.id == id) {
            return Err("Only installed third-party packages can be removed".into());
        }
        let mut next = self.clone();
        next.setting_values.remove(id);
        next.policy.packages.retain(|p| p.manifest.id != id);
        next.cache.remove(id);
        next.memory_packages.remove(id);
        next.policy.disabled.remove(id);
        next.epoch = next
            .epoch
            .checked_add(1)
            .ok_or("Extension generation exhausted")?;
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
    pub fn overlay(&self, id: &str) -> bool {
        self.manifest(id)
            .is_some_and(|m| m.capabilities.contains(&Capability::WindowOverlay))
    }
    pub fn module(&self, id: &str) -> Option<(u64, Arc<crate::sandbox::Compiled>)> {
        if self.policy.disabled.contains(id) {
            return None;
        }
        self.cache.get(id).map(|p| (p.generation, p.module.clone()))
    }
    pub fn allows(&self, id: &str, account: &str, capability: Capability) -> bool {
        self.list(account).iter().any(|state| {
            state.manifest.id == id
                && state.enabled
                && state.manifest.capabilities.contains(&capability)
        })
    }
    pub fn allows_guest(
        &self,
        id: &str,
        account: &str,
        capability: Capability,
        epoch: u64,
    ) -> bool {
        self.cache.get(id).is_some_and(|p| p.generation == epoch)
            && self.policy.packages.iter().any(|p| p.manifest.id == id)
            && self.allows(id, account, capability)
    }
    pub fn bundled_identity(&self, id: &str) -> bool {
        self.manifests.iter().any(|m| m.id == id)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn duplicate_ids_and_bus_subscriptions() {
        let mut extensions = Extensions::bundled();
        let base =
            include_str!("../../../docs/extensions/examples/connection-panel/extension.toml");
        let module = wat::parse_str("(module (func (export \"run\")))".replace("\\", "")).unwrap();
        for id in ["reader", "connection-panel"] {
            let source = base.replace("connection-panel", id);
            if id == "connection-panel" {
                extensions.install(&source, module.clone()).unwrap();
            }
            assert_eq!(
                extensions.install(&source, module.clone()).unwrap_err(),
                format!("An extension with the ID `{id}` is already installed")
            );
        }
        let bus = base.replace("account.read", "extensions.bus");
        assert!(
            parse_manifest(&format!(
                "{bus}\n[bus]\nsubscribe = ['reader/v1/message_opened']"
            ))
            .is_ok()
        );
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
    fn compiled_cache_reuses_modules_but_instances_are_fresh_and_reinstall_invalidates() {
        let mut extensions = Extensions::bundled();
        let source =
            include_str!("../../../docs/extensions/examples/connection-panel/extension.toml");
        let view = r#"{"title":"Fresh","text":"fresh","actions":[]}"#;
        let escaped: String = view.bytes().map(|b| format!("\\{b:02x}")).collect();
        let module = wat::parse_str(format!(
            r#"(module
            (import "colecto" "emit" (func $emit (param i32 i32) (result i32)))
            (global $counter (mut i32) (i32.const 0))
            (memory (export "memory") 1) (data (i32.const 0) "{escaped}")
            (func (export "run")
                global.get $counter if unreachable end
                i32.const 1 global.set $counter
                i32.const 0 i32.const {} call $emit drop))"#,
            view.len()
        ))
        .unwrap();
        extensions.install(source, module.clone()).unwrap();
        let (generation, first) = extensions.module("connection-panel").unwrap();
        for _ in 0..2 {
            let (_, cached) = extensions.module("connection-panel").unwrap();
            assert!(Arc::ptr_eq(&first, &cached));
            assert_eq!(
                crate::sandbox::run_output(&cached, "open", Box::new(|_| unreachable!()), None)
                    .unwrap()["text"],
                "fresh"
            );
        }
        extensions.update("connection-panel", false).unwrap();
        assert!(!extensions.cache.contains_key("connection-panel"));
        extensions.remove("connection-panel").unwrap();
        extensions.install(source, module).unwrap();
        let (next, reinstalled) = extensions.module("connection-panel").unwrap();
        assert_ne!(generation, next);
        assert!(!Arc::ptr_eq(&first, &reinstalled));
        assert!(!extensions.allows_guest(
            "connection-panel",
            "a",
            Capability::AccountRead,
            generation
        ));
    }
    #[test]
    fn installed_package_count_boundary() {
        let mut extensions = Extensions::bundled();
        let source =
            include_str!("../../../docs/extensions/examples/connection-panel/extension.toml");
        let module = wat::parse_str("(module (func (export \"run\")))").unwrap();
        for index in 0..=MAX_INSTALLED {
            let installed = extensions.install(
                &source.replace("connection-panel", &format!("example-{index}")),
                module.clone(),
            );
            assert_eq!(installed.is_ok(), index < MAX_INSTALLED);
            if index >= MAX_INSTALLED - 2 {
                assert_eq!(
                    extensions.policy.packages.len(),
                    (index + 1).min(MAX_INSTALLED)
                );
            }
        }
    }
    #[test]
    fn overlay_manifest_requires_exact_layer_contract() {
        let source = include_str!("../../../ui/src/extensions/notifications/extension.toml");
        assert!(parse_manifest(source).is_ok());
        for invalid in [
            source.replace("z_index = 500", ""),
            source.replace("500", "0"),
            source.replace("500", "10001"),
            source.replace("window.overlay", "account.read"),
            source.replace("window.overlay", "app.navigate"),
            format!("{source}\nunknown = true"),
        ] {
            assert!(parse_manifest(&invalid).is_err(), "{invalid}");
        }
    }
    #[test]
    fn enabled_policy_and_packages_survive_restart() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::open(dir.path()).unwrap();
        let open = || Extensions::open(database.clone()).unwrap();
        let mut extensions = open();
        let source =
            include_str!("../../../docs/extensions/examples/connection-panel/extension.toml");
        extensions
            .install(
                source,
                wat::parse_str("(module (func (export \"run\")))").unwrap(),
            )
            .unwrap();
        assert!(open().allows("connection-panel", "new-account", Capability::AccountRead));
        extensions.update("connection-panel", false).unwrap();
        let mut restored = open();
        assert!(!restored.allows("connection-panel", "new-account", Capability::AccountRead));
        restored.remove("connection-panel").unwrap();
        assert!(open().module("connection-panel").is_none());
        for invalid in [
            source.replace("api_version = 1", "api_version = 2"),
            source.replace("account.read", "accounts.manage"),
            source.replace("connection-panel", "../escape"),
        ] {
            assert!(parse_manifest(&invalid).is_err());
        }
    }
    #[test]
    fn install_enables_declared_access_everywhere_and_disable_revokes() {
        let mut extensions = Extensions::bundled();
        extensions
            .install(
                include_str!("../../../docs/extensions/examples/connection-panel/extension.toml"),
                wat::parse_str("(module (func (export \"run\")))").unwrap(),
            )
            .unwrap();
        let epoch = extensions.module("connection-panel").unwrap().0;
        for account in ["a", "b"] {
            assert!(extensions.allows("connection-panel", account, Capability::AccountRead));
            assert!(!extensions.allows("connection-panel", account, Capability::MessagesRead));
        }
        extensions.update("connection-panel", false).unwrap();
        assert!(!extensions.allows("connection-panel", "a", Capability::AccountRead));
        extensions.update("connection-panel", true).unwrap();
        assert!(!extensions.allows_guest("connection-panel", "a", Capability::AccountRead, epoch));
        let old =
            br#"{"version":1,"disabled":[],"grants":[{"nonsense":"obsolete"}],"packages":[]}"#;
        let policy: Policy = serde_json::from_slice(old).unwrap();
        extensions.check(&policy).unwrap();
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
            include_str!("../../../docs/extensions/examples/connection-panel/extension.toml")
        );
        let module = wat::parse_str(include_str!(
            "../../../docs/extensions/examples/connection-panel/module.wat"
        ))
        .unwrap();
        extensions.install(&source, module.clone()).unwrap();
        for value in [
            serde_json::json!({"speed":31}),
            serde_json::json!({"speed":"10"}),
            serde_json::json!({"unknown":10}),
        ] {
            assert!(
                extensions
                    .update_settings("connection-panel", value.as_object().unwrap().clone())
                    .is_err()
            );
        }
        extensions
            .update_settings(
                "connection-panel",
                serde_json::json!({"speed":12}).as_object().unwrap().clone(),
            )
            .unwrap();
        assert_eq!(
            Extensions::open(database.clone())
                .unwrap()
                .settings("connection-panel")
                .unwrap()["values"]["speed"],
            12
        );
        extensions.remove("connection-panel").unwrap();
        extensions.install(&source, module).unwrap();
        assert_eq!(
            extensions.settings("connection-panel").unwrap()["values"]["speed"],
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
