//! Owner preferences: the workspace layout, the selected account, whether MCP is on,
//! the accent colour and notification choices.
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
    /// `#rrggbb`; absent means the default mint.
    pub accent: Option<String>,
    pub notifications: Notifications,
}

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "kebab-case")]
pub enum Corner {
    TopLeft,
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
}

/// In-app notifications; `system` also uses the desktop's notifications while Colecto is unfocused.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct Notifications {
    pub enabled: bool,
    pub mail: bool,
    pub approvals: bool,
    pub system: bool,
    pub position: Corner,
}
impl Default for Notifications {
    fn default() -> Self {
        Self {
            enabled: true,
            mail: true,
            approvals: true,
            system: false,
            position: Corner::BottomRight,
        }
    }
}

/// Fields that are present replace the stored ones.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreferencesInput {
    pub mcp: Option<bool>,
    pub account: Option<String>,
    pub workspace: Option<serde_json::Value>,
    /// An empty string restores the default accent.
    pub accent: Option<String>,
    pub notifications: Option<Notifications>,
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
    if preferences.accent.as_ref().is_some_and(|accent| {
        accent.len() != 7
            || !accent.starts_with('#')
            || !accent[1..].bytes().all(|b| b.is_ascii_hexdigit())
    }) {
        return Err("Accent must be a #rrggbb colour".into());
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
        if let Some(accent) = input.accent {
            next.accent = (!accent.is_empty()).then_some(accent.to_ascii_lowercase());
        }
        if let Some(notifications) = input.notifications {
            next.notifications = notifications;
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
            accent: None,
            notifications: None,
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

    #[test]
    fn accent_and_notifications_are_validated_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::open(dir.path()).unwrap();
        let mut store = PreferenceStore::open(database.clone(), dir.path()).unwrap();
        assert_eq!(store.get().notifications, Notifications::default());
        let accent = |value: &str| PreferencesInput {
            mcp: None,
            account: None,
            workspace: None,
            accent: Some(value.into()),
            notifications: None,
        };
        for invalid in ["red", "#12345", "#1234567", "#12345g", "url(#a)"] {
            assert!(store.update(accent(invalid)).is_err(), "{invalid}");
        }
        store.update(accent("#7AA2F7")).unwrap();
        let notifications: Notifications = serde_json::from_value(serde_json::json!({
            "enabled": true, "mail": false, "approvals": true, "system": true, "position": "top-left"
        }))
        .unwrap();
        store
            .update(PreferencesInput {
                notifications: Some(notifications.clone()),
                ..accent("#7aa2f7")
            })
            .unwrap();
        assert!(
            serde_json::from_value::<Notifications>(serde_json::json!({"position": "middle"}))
                .is_err()
        );
        let mut store = PreferenceStore::open(database, dir.path()).unwrap();
        assert_eq!(store.get().accent.as_deref(), Some("#7aa2f7"));
        assert_eq!(store.get().notifications, notifications);
        store.update(accent("")).unwrap();
        assert_eq!(store.get().accent, None);
    }
}
