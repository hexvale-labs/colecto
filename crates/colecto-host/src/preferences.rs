//! Owner preferences: the workspace layout, the selected account, whether MCP is on,
//! the accent colour, notification choices and whether mail loads remote images.
//! The UI owns the layout's shape; the Host only bounds it, because it never acts on it.
use colecto_core::Database;
use serde::{Deserialize, Serialize};

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
    pub font: Option<String>,
    pub font_size: Option<u8>,
    pub notifications: Notifications,
    notification_position_chosen: bool,
    /// Load remote images in HTML mail without asking. Off by default: they can track opens.
    pub remote_images: bool,
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
            position: Corner::Top,
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
    pub font: Option<String>,
    pub font_size: Option<u8>,
    pub notifications: Option<Notifications>,
    pub remote_images: Option<bool>,
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
    if preferences.font.as_ref().is_some_and(|font| {
        font.trim().is_empty()
            || font.chars().count() > 64
            || !font
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.'))
    }) {
        return Err("Font must be a bundled id or system family of at most 64 characters".into());
    }
    if preferences
        .font_size
        .is_some_and(|size| !(13..=18).contains(&size))
    {
        return Err("Font size must be between 13 and 18".into());
    }
    Ok(())
}

pub struct PreferenceStore {
    database: Database,
    value: Preferences,
}
impl PreferenceStore {
    pub fn open(database: Database) -> Result<Self, String> {
        let value = crate::store::open(&database, "preferences", Preferences::default, |bytes| {
            let mut value: Preferences =
                serde_json::from_slice(bytes).map_err(|_| "Invalid preferences")?;
            if !value.notification_position_chosen
                && value.notifications.position == Corner::BottomRight
            {
                value.notifications.position = Corner::Top;
            }
            validate(&value)?;
            Ok(value)
        })?;
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
        if let Some(font) = input.font {
            next.font = Some(font);
        }
        if let Some(size) = input.font_size {
            next.font_size = Some(size);
        }
        if let Some(notifications) = input.notifications {
            next.notification_position_chosen |=
                notifications.position != next.notifications.position;
            next.notifications = notifications;
        }
        if let Some(remote_images) = input.remote_images {
            next.remote_images = remote_images;
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
    fn typography_is_bounded_and_persists() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();
        let mut store = PreferenceStore::open(db.clone()).unwrap();
        for value in [
            serde_json::json!({"font":""}),
            serde_json::json!({"font":"   "}),
            serde_json::json!({"font":"x".repeat(65)}),
            serde_json::json!({"font":"Arial; color:red"}),
            serde_json::json!({"font_size":12}),
            serde_json::json!({"font_size":19}),
        ] {
            assert!(
                store
                    .update(serde_json::from_value(value).unwrap())
                    .is_err()
            );
        }
        for font in ["mona", "inter", "geist", "plex", "atkinson", "Segoe UI"] {
            store
                .update(
                    serde_json::from_value(serde_json::json!({"font":font,"font_size":18}))
                        .unwrap(),
                )
                .unwrap();
        }
        let restored = PreferenceStore::open(db).unwrap();
        assert_eq!(restored.get().font.as_deref(), Some("Segoe UI"));
        assert_eq!(restored.get().font_size, Some(18));
    }
    #[test]
    fn legacy_default_moves_to_top_but_new_position_choices_persist() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::open(dir.path()).unwrap();
        crate::store::save(
            &database,
            "preferences",
            &serde_json::json!({"notifications":{"position":"bottom-right"}}),
        )
        .unwrap();
        let mut store = PreferenceStore::open(database.clone()).unwrap();
        assert_eq!(store.get().notifications.position, Corner::Top);
        store
            .update(
                serde_json::from_value(
                    serde_json::json!({"notifications":{"position":"bottom-right"}}),
                )
                .unwrap(),
            )
            .unwrap();
        assert_eq!(
            PreferenceStore::open(database)
                .unwrap()
                .get()
                .notifications
                .position,
            Corner::BottomRight
        );
    }
    #[test]
    fn mcp_starts_off_and_preferences_survive_restart_within_bounds() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::open(dir.path()).unwrap();
        let mut store = PreferenceStore::open(database.clone()).unwrap();
        assert!(!store.get().mcp);
        let input = |workspace: serde_json::Value| PreferencesInput {
            mcp: Some(true),
            account: Some("work".into()),
            workspace: Some(workspace),
            accent: None,
            font: None,
            font_size: None,
            notifications: None,
            remote_images: Some(true),
        };
        assert!(!store.get().remote_images);
        store
            .update(input(serde_json::json!({"root": {}})))
            .unwrap();
        assert!(store.update(input(serde_json::json!([1]))).is_err());
        let big = serde_json::json!({"x": "a".repeat(MAX_WORKSPACE_BYTES)});
        assert!(store.update(input(big)).is_err());
        let store = PreferenceStore::open(database).unwrap();
        assert!(store.get().mcp);
        assert_eq!(store.get().account.as_deref(), Some("work"));
        assert_eq!(store.get().workspace, Some(serde_json::json!({"root": {}})));
        assert!(store.get().remote_images);
    }

    #[test]
    fn accent_and_notifications_are_validated_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::open(dir.path()).unwrap();
        let mut store = PreferenceStore::open(database.clone()).unwrap();
        assert_eq!(store.get().notifications, Notifications::default());
        let accent = |value: &str| PreferencesInput {
            mcp: None,
            account: None,
            workspace: None,
            accent: Some(value.into()),
            font: None,
            font_size: None,
            notifications: None,
            remote_images: None,
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
        let mut store = PreferenceStore::open(database).unwrap();
        assert_eq!(store.get().accent.as_deref(), Some("#7aa2f7"));
        assert_eq!(store.get().notifications, notifications);
        store.update(accent("")).unwrap();
        assert_eq!(store.get().accent, None);
    }
}
