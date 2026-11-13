//! Owner preferences: the workspace layout, the selected account, whether MCP is on,
//! the accent colour, notification choices, whether mail loads remote images and how
//! the message list looks, which accounts are on and how much mail the Host downloads
//! ahead of time.
//! The UI owns the layout's shape; the Host only bounds it, because it never acts on it.
use colecto_core::Database;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const MAX_WORKSPACE_BYTES: usize = 64 * 1024;

#[derive(Clone, Serialize, Deserialize)]
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
    /// Per-account on/off choices. Accounts missing here use [`default_enabled`]: the demo
    /// account is off, every other account on. Off accounts are hidden from the workspace and
    /// never refreshed, watched or prefetched in the background.
    pub accounts_enabled: BTreeMap<String, bool>,
    /// Set once the owner finished or skipped first-run onboarding.
    pub onboarded: bool,
    pub list_density: ListDensity,
    /// Group the message list under "Today", "Yesterday", month names and so on.
    pub date_groups: bool,
    /// The newest release the owner was already told about, so each version notifies once.
    pub notified_release: Option<String>,
    pub prefetch: Prefetch,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            mcp: false,
            account: None,
            workspace: None,
            accent: None,
            font: None,
            font_size: None,
            notifications: Notifications::default(),
            notification_position_chosen: false,
            remote_images: false,
            list_density: ListDensity::Full,
            date_groups: true,
            notified_release: None,
            accounts_enabled: BTreeMap::new(),
            onboarded: false,
            prefetch: Prefetch::default(),
        }
    }
}

/// Message list rows: two lines with a preview, a shorter version, or one line.
#[derive(Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum ListDensity {
    #[default]
    Full,
    Medium,
    Compact,
}

const MAX_ACCOUNT_CHOICES: usize = 256;
/// Whether an account the owner never switched on or off is enabled.
pub fn default_enabled(id: &str) -> bool {
    id != colecto_extension_api::DEMO_ACCOUNT
}
impl Preferences {
    pub fn account_enabled(&self, id: &str) -> bool {
        self.accounts_enabled
            .get(id)
            .copied()
            .unwrap_or_else(|| default_enabled(id))
    }
}

/// After each Inbox refresh, download the newest `count` messages that are not cached yet.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct Prefetch {
    pub enabled: bool,
    pub count: u16,
}
impl Default for Prefetch {
    fn default() -> Self {
        Self {
            enabled: true,
            count: 50,
        }
    }
}
/// A refresh lists at most this many messages, so prefetch can reach no further.
pub const MAX_PREFETCH_COUNT: u16 = colecto_core::MAX_LIST_MESSAGES as u16;

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
    /// Tell the owner when a newer release is published. Off means the UI never asks GitHub.
    pub updates: bool,
}
impl Default for Notifications {
    fn default() -> Self {
        Self {
            enabled: true,
            mail: true,
            approvals: true,
            system: false,
            position: Corner::Top,
            updates: true,
        }
    }
}

/// Fields that are present replace the stored ones; `accounts_enabled` entries are merged.
#[derive(Default, Deserialize)]
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
    pub accounts_enabled: Option<BTreeMap<String, bool>>,
    pub onboarded: Option<bool>,
    pub list_density: Option<ListDensity>,
    pub date_groups: Option<bool>,
    pub notified_release: Option<String>,
    pub prefetch: Option<Prefetch>,
}

fn validate(preferences: &Preferences) -> Result<(), String> {
    if preferences
        .account
        .as_ref()
        .is_some_and(|a| a.is_empty() || a.len() > 128)
    {
        return Err("Invalid account preference".into());
    }
    if preferences.accounts_enabled.len() > MAX_ACCOUNT_CHOICES
        || preferences
            .accounts_enabled
            .keys()
            .any(|id| id.is_empty() || id.len() > 128)
    {
        return Err("Invalid account on/off choices".into());
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
    if preferences
        .notified_release
        .as_ref()
        .is_some_and(|version| {
            version.is_empty()
                || version.len() > 32
                || !version.bytes().all(|b| b.is_ascii_digit() || b == b'.')
        })
    {
        return Err("Release version must look like 1.2.3".into());
    }
    if preferences.prefetch.count > MAX_PREFETCH_COUNT {
        return Err(format!(
            "Prefetch count must be between 0 and {MAX_PREFETCH_COUNT}"
        ));
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
    /// Forget a removed account's selection and on/off choice.
    pub fn clear_account(&mut self, id: &str) -> Result<(), String> {
        let mut next = self.value.clone();
        if next.account.as_deref() == Some(id) {
            next.account = None;
        }
        next.accounts_enabled.remove(id);
        if next.account != self.value.account
            || next.accounts_enabled.len() != self.value.accounts_enabled.len()
        {
            crate::store::save(&self.database, "preferences", &next)?;
            self.value = next;
        }
        Ok(())
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
        if let Some(choices) = input.accounts_enabled {
            next.accounts_enabled.extend(choices);
        }
        if let Some(onboarded) = input.onboarded {
            next.onboarded = onboarded;
        }
        if let Some(density) = input.list_density {
            next.list_density = density;
        }
        if let Some(date_groups) = input.date_groups {
            next.date_groups = date_groups;
        }
        if let Some(version) = input.notified_release {
            next.notified_release = Some(version);
        }
        if let Some(prefetch) = input.prefetch {
            next.prefetch = prefetch;
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
    fn list_and_update_preferences_default_validate_and_persist() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::open(dir.path()).unwrap();
        // Preferences saved before these fields existed keep the defaults.
        crate::store::save(
            &database,
            "preferences",
            &serde_json::json!({"notifications":{"enabled":true,"mail":true,"approvals":true,"system":false,"position":"top"}}),
        )
        .unwrap();
        let mut store = PreferenceStore::open(database.clone()).unwrap();
        assert_eq!(store.get().list_density, ListDensity::Full);
        assert!(store.get().date_groups);
        assert!(store.get().notifications.updates);
        assert_eq!(store.get().notified_release, None);
        for value in [
            serde_json::json!({"list_density":"tiny"}),
            serde_json::json!({"date_groups":"yes"}),
            serde_json::json!({"notified_release":""}),
            serde_json::json!({"notified_release":"1.2.3<script>"}),
        ] {
            assert!(
                serde_json::from_value::<PreferencesInput>(value)
                    .map_err(|e| e.to_string())
                    .and_then(|input| store.update(input))
                    .is_err()
            );
        }
        store
            .update(
                serde_json::from_value(serde_json::json!({
                    "list_density":"compact","date_groups":false,"notified_release":"0.4.0",
                    "notifications":{"updates":false}
                }))
                .unwrap(),
            )
            .unwrap();
        let restored = PreferenceStore::open(database).unwrap();
        assert_eq!(restored.get().list_density, ListDensity::Compact);
        assert!(!restored.get().date_groups);
        assert_eq!(restored.get().notified_release.as_deref(), Some("0.4.0"));
        assert!(!restored.get().notifications.updates);
        assert!(restored.get().notifications.enabled);
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
            remote_images: Some(true),
            ..Default::default()
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
    fn prefetch_defaults_on_and_is_bounded() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::open(dir.path()).unwrap();
        let mut store = PreferenceStore::open(database.clone()).unwrap();
        assert_eq!(store.get().prefetch, Prefetch::default());
        assert!(store.get().prefetch.enabled);
        let input = |value| serde_json::from_value::<PreferencesInput>(value).unwrap();
        assert!(
            store
                .update(input(serde_json::json!({"prefetch": {"enabled": true, "count": MAX_PREFETCH_COUNT + 1}})))
                .is_err()
        );
        assert!(
            serde_json::from_value::<PreferencesInput>(
                serde_json::json!({"prefetch": {"count": 70000}})
            )
            .is_err()
        );
        store
            .update(input(
                serde_json::json!({"prefetch": {"enabled": false, "count": 10}}),
            ))
            .unwrap();
        let store = PreferenceStore::open(database).unwrap();
        assert_eq!(
            store.get().prefetch,
            Prefetch {
                enabled: false,
                count: 10
            }
        );
    }
    #[test]
    fn accent_and_notifications_are_validated_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::open(dir.path()).unwrap();
        let mut store = PreferenceStore::open(database.clone()).unwrap();
        assert_eq!(store.get().notifications, Notifications::default());
        let accent = |value: &str| PreferencesInput {
            accent: Some(value.into()),
            ..Default::default()
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

    #[test]
    fn demo_starts_off_other_accounts_on_and_choices_merge() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::open(dir.path()).unwrap();
        let mut store = PreferenceStore::open(database.clone()).unwrap();
        assert!(!store.get().onboarded);
        assert!(!store.get().account_enabled("demo"));
        assert!(store.get().account_enabled("a1"));
        let choose = |id: &str, on: bool| PreferencesInput {
            accounts_enabled: Some([(id.to_owned(), on)].into()),
            ..Default::default()
        };
        store.update(choose("demo", true)).unwrap();
        store.update(choose("a1", false)).unwrap();
        assert!(store.update(choose("", false)).is_err());
        let mut store = PreferenceStore::open(database).unwrap();
        assert!(store.get().account_enabled("demo"));
        assert!(!store.get().account_enabled("a1"));
        store.clear_account("a1").unwrap();
        assert!(store.get().account_enabled("a1"));
    }
}
