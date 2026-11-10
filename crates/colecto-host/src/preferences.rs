//! Owner preferences: the workspace layout, the selected account and whether MCP is on.
//! The UI owns the layout's shape; the Host only bounds it, because it never acts on it.
use colecto_core::Database;
use serde::{Deserialize, Serialize};
use std::path::Path;

const MAX_WORKSPACE_BYTES: usize = 64 * 1024;

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Preferences {
    /// The MCP endpoint answers only after the owner turns it on in Settings.
    pub mcp: bool,
    pub account: Option<String>,
    pub workspace: Option<serde_json::Value>,
}

/// Fields that are present replace the stored ones.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreferencesInput {
    pub mcp: Option<bool>,
    pub account: Option<String>,
    pub workspace: Option<serde_json::Value>,
}

fn validate(preferences: &Preferences) -> Result<(), String> {
    if preferences
        .account
        .as_ref()
        .is_some_and(|a| a.is_empty() || a.len() > 128)
    {
        return Err("Invalid account preference".into());
    }
    if let Some(workspace) = &preferences.workspace
        && (!workspace.is_object()
            || serde_json::to_vec(workspace).map_or(true, |b| b.len() > MAX_WORKSPACE_BYTES))
    {
        return Err("Workspace layout must be an object of at most 64 KiB".into());
    }
    Ok(())
}

pub struct PreferenceStore {
    database: Database,
    value: Preferences,
}
impl PreferenceStore {
    pub fn open(database: Database, directory: &Path) -> Result<Self, String> {
        let value = crate::store::open(
            &database,
            directory,
            "preferences",
            None,
            Preferences::default,
            |bytes| {
                let value: Preferences =
                    serde_json::from_slice(bytes).map_err(|_| "Invalid preferences")?;
                validate(&value)?;
                Ok(value)
            },
        )?;
        Ok(Self { database, value })
    }
    pub fn get(&self) -> &Preferences {
        &self.value
    }
    pub fn update(&mut self, input: PreferencesInput) -> Result<Preferences, String> {
        let mut next = self.value.clone();
        if let Some(mcp) = input.mcp {
            next.mcp = mcp;
        }
        if let Some(account) = input.account {
            next.account = Some(account);
        }
        if let Some(workspace) = input.workspace {
            next.workspace = Some(workspace);
        }
        validate(&next)?;
        crate::store::save(&self.database, "preferences", &next)?;
        self.value = next;
        Ok(self.value.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_starts_off_and_preferences_survive_restart_within_bounds() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::open(dir.path()).unwrap();
        let mut store = PreferenceStore::open(database.clone(), dir.path()).unwrap();
        assert!(!store.get().mcp);
        let input = |workspace: serde_json::Value| PreferencesInput {
            mcp: Some(true),
            account: Some("work".into()),
            workspace: Some(workspace),
        };
        store
            .update(input(serde_json::json!({"root": {}})))
            .unwrap();
        assert!(store.update(input(serde_json::json!([1]))).is_err());
        let big = serde_json::json!({"x": "a".repeat(MAX_WORKSPACE_BYTES)});
        assert!(store.update(input(big)).is_err());
        let store = PreferenceStore::open(database, dir.path()).unwrap();
        assert!(store.get().mcp);
        assert_eq!(store.get().account.as_deref(), Some("work"));
        assert_eq!(store.get().workspace, Some(serde_json::json!({"root": {}})));
    }
}
