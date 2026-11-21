//! Owner preferences: the workspace layout, the selected account, whether MCP is on,
//! the accent colour, notification choices, whether mail loads remote images and how
//! the message list looks, which accounts are on and how much mail the service downloads
//! ahead of time.
use colecto_core::{DEMO_ACCOUNT, Database, MAX_LIST_MESSAGES};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const MAX_WORKSPACE_BYTES: usize = 64 * 1024;

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
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
    /// The UI design system, one of [`DESIGNS`]; absent means the platform's own.
    pub design: Option<String>,
    /// Whole-interface zoom in percent ([`ZOOM_RANGE`]); absent means 100 %.
    pub zoom: Option<u16>,
    pub text_spacing: TextSpacing,
    pub notifications: Notifications,
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
            design: None,
            zoom: None,
            text_spacing: TextSpacing::default(),
            notifications: Notifications::default(),
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
    id != DEMO_ACCOUNT
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

/// Extra space around text, which helps many readers with dyslexia more than a special
/// font does. Each is a step from 0 (as designed) to [`MAX_SPACING_STEP`]; the UI maps the
/// steps to lengths, the largest matching WCAG 1.4.12 (Text Spacing).
#[derive(Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct TextSpacing {
    pub letter: u8,
    pub word: u8,
    pub line: u8,
    pub paragraph: u8,
}

pub const MAX_SPACING_STEP: u8 = 2;

/// A refresh lists at most this many messages, so prefetch can reach no further.
pub const MAX_PREFETCH_COUNT: u16 = MAX_LIST_MESSAGES as u16;

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

/// How notifications show; `system` also uses the desktop's notifications while Colecto is
/// unfocused.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
#[serde(default, deny_unknown_fields)]
pub struct Notifications {
    pub enabled: bool,
    pub system: bool,
    pub position: Corner,
}

impl Default for Notifications {
    fn default() -> Self {
        Self {
            enabled: true,
            system: false,
            position: Corner::Top,
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
    /// An empty string restores the platform font.
    pub font: Option<String>,
    pub font_size: Option<u8>,
    /// An empty string restores the platform's own design.
    pub design: Option<String>,
    /// 100 restores the default zoom.
    pub zoom: Option<u16>,
    pub text_spacing: Option<TextSpacing>,
    pub notifications: Option<Notifications>,
    pub remote_images: Option<bool>,
    pub accounts_enabled: Option<BTreeMap<String, bool>>,
    pub onboarded: Option<bool>,
    pub list_density: Option<ListDensity>,
    pub date_groups: Option<bool>,
    pub notified_release: Option<String>,
    pub prefetch: Option<Prefetch>,
    /// Setting keys, such as `notifications.position`, to return to their defaults.
    #[serde(default)]
    pub reset: Vec<String>,
}

/// Design systems the Flutter UI can draw with: shadcn (Linux and Web), Fluent (Windows),
/// macOS, Material 3 (Android) and Cupertino (iOS). Any design works on any platform.
/// Zoom the owner can pick with Ctrl/Cmd +, - and 0, in percent.
pub const ZOOM_RANGE: std::ops::RangeInclusive<u16> = 50..=200;

pub const DESIGNS: [&str; 5] = ["colecto", "fluent", "macos", "material", "cupertino"];

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
        .design
        .as_deref()
        .is_some_and(|design| !DESIGNS.contains(&design))
    {
        return Err(format!("Design must be one of {}", DESIGNS.join(", ")));
    }
    if preferences
        .zoom
        .is_some_and(|zoom| !ZOOM_RANGE.contains(&zoom))
    {
        return Err(format!(
            "Zoom must be between {}% and {}%",
            ZOOM_RANGE.start(),
            ZOOM_RANGE.end()
        ));
    }
    if preferences
        .font_size
        .is_some_and(|size| !(13..=18).contains(&size))
    {
        return Err("Font size must be between 13 and 18".into());
    }
    let spacing = preferences.text_spacing;
    if [
        spacing.letter,
        spacing.word,
        spacing.line,
        spacing.paragraph,
    ]
    .iter()
    .any(|step| *step > MAX_SPACING_STEP)
    {
        return Err(format!(
            "Text spacing steps must be between 0 and {MAX_SPACING_STEP}"
        ));
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

/// Settings scope of the owner preferences in the `settings` table.
const SCOPE: &str = "core";

/// What `/api/v1/preferences` returns: the preferences, and the setting keys the owner
/// changed, so Settings can offer to reset them.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct PreferencesView {
    #[serde(flatten)]
    pub preferences: Preferences,
    pub changed: BTreeSet<String>,
}

pub struct PreferenceStore {
    database: Database,
    value: Preferences,
    changed: BTreeSet<String>,
}

impl PreferenceStore {
    pub fn open(database: Database) -> Result<Self, String> {
        let (value, changed) = crate::store::load_settings(&database, SCOPE, validate)?;
        Ok(Self {
            database,
            value,
            changed,
        })
    }

    fn save(&mut self, next: Preferences, reset: &[String]) -> Result<(), String> {
        crate::store::save_settings(&self.database, SCOPE, &self.value, &next, reset)?;
        (self.value, self.changed) = crate::store::load_settings(&self.database, SCOPE, validate)?;
        Ok(())
    }

    pub fn get(&self) -> &Preferences {
        &self.value
    }

    pub fn view(&self) -> PreferencesView {
        PreferencesView {
            preferences: self.value.clone(),
            changed: self.changed.clone(),
        }
    }

    /// Forget a removed account's selection and on/off choice.
    pub fn clear_account(&mut self, id: &str) -> Result<(), String> {
        let mut next = self.value.clone();
        if next.account.as_deref() == Some(id) {
            next.account = None;
        }
        next.accounts_enabled.remove(id);
        self.save(next, &[])
    }

    pub fn update(&mut self, input: PreferencesInput) -> Result<Preferences, String> {
        let mut next = self.value.clone();
        let mut reset = input.reset;
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
            if accent.is_empty() {
                reset.push("accent".into());
            } else {
                next.accent = Some(accent.to_ascii_lowercase());
            }
        }
        if let Some(font) = input.font {
            if font.is_empty() {
                reset.push("font".into());
            } else {
                next.font = Some(font);
            }
        }
        if let Some(zoom) = input.zoom {
            if zoom == 100 {
                reset.push("zoom".into());
            } else {
                next.zoom = Some(zoom);
            }
        }
        if let Some(design) = input.design {
            if design.is_empty() {
                reset.push("design".into());
            } else {
                next.design = Some(design);
            }
        }
        if let Some(size) = input.font_size {
            next.font_size = Some(size);
        }
        if let Some(spacing) = input.text_spacing {
            next.text_spacing = spacing;
        }
        if let Some(notifications) = input.notifications {
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
        self.save(next, &reset)?;
        Ok(self.value.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_bounds_reject_invalid_values_and_accept_valid_presets() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();
        let mut store = PreferenceStore::open(db).unwrap();

        for invalid in [
            serde_json::json!({"design":"aqua"}),
            serde_json::json!({"zoom":49}),
            serde_json::json!({"zoom":201}),
            serde_json::json!({"font":"   "}),
            serde_json::json!({"font":"x".repeat(65)}),
            serde_json::json!({"font":"Arial; color:red"}),
            serde_json::json!({"font_size":12}),
            serde_json::json!({"font_size":19}),
            serde_json::json!({"text_spacing":{"letter":3}}),
            serde_json::json!({"text_spacing":{"lines":1}}),
            serde_json::json!({"list_density":"tiny"}),
            serde_json::json!({"date_groups":"yes"}),
            serde_json::json!({"notified_release":""}),
            serde_json::json!({"notified_release":"1.2.3<script>"}),
            serde_json::json!({"notifications":{"updates":false}}),
            serde_json::json!({"notifications":{"position":"middle"}}),
            serde_json::json!({"reset":["nonexistent"]}),
            serde_json::json!({"workspace":[1]}),
            serde_json::json!({"workspace":{"x":"a".repeat(MAX_WORKSPACE_BYTES)}}),
            serde_json::json!({"prefetch":{"enabled":true,"count":MAX_PREFETCH_COUNT + 1}}),
            serde_json::json!({"prefetch":{"count":70000}}),
            serde_json::json!({"accent":"red"}),
            serde_json::json!({"accent":"#12345"}),
            serde_json::json!({"accent":"#1234567"}),
            serde_json::json!({"accent":"#12345g"}),
            serde_json::json!({"accent":"url(#a)"}),
            serde_json::json!({"accounts_enabled":{"":false}}),
        ] {
            assert!(
                serde_json::from_value::<PreferencesInput>(invalid.clone())
                    .map_err(|e| e.to_string())
                    .and_then(|input| store.update(input))
                    .is_err(),
                "expected invalid: {invalid}"
            );
        }

        for design in DESIGNS {
            let input: PreferencesInput =
                serde_json::from_value(serde_json::json!({ "design": design })).unwrap();
            assert_eq!(store.update(input).unwrap().design.as_deref(), Some(design));
        }
        for font in ["mona", "inter", "geist", "plex", "atkinson", "Segoe UI"] {
            store
                .update(
                    serde_json::from_value(serde_json::json!({"font":font,"font_size":18}))
                        .unwrap(),
                )
                .unwrap();
        }
    }

    #[test]
    fn only_changed_preferences_are_stored_and_reset_clears_them() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::open(dir.path()).unwrap();
        let mut store = PreferenceStore::open(database.clone()).unwrap();
        assert!(database.settings(SCOPE).unwrap().is_empty());

        // The UI sends the whole notifications object; only the field that changed is kept.
        store
            .update(
                serde_json::from_value(serde_json::json!({
                    "notifications":{"enabled":true,"system":false,"position":"bottom-right"},
                    "accent":"#AABBCC",
                    "design":"fluent",
                    "zoom":125
                }))
                .unwrap(),
            )
            .unwrap();
        let keys: Vec<String> = database
            .settings(SCOPE)
            .unwrap()
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        assert_eq!(keys, ["accent", "design", "notifications.position", "zoom"]);
        assert_eq!(store.view().changed, keys.iter().cloned().collect());
        assert_eq!(store.get().accent.as_deref(), Some("#aabbcc"));

        // Empty values, default zoom (100), and `reset` all return to defaults and prune storage.
        let value = store
            .update(
                serde_json::from_value(serde_json::json!({
                    "accent":"",
                    "design":"",
                    "font":"",
                    "zoom":100,
                    "reset":["notifications.position"]
                }))
                .unwrap(),
            )
            .unwrap();
        assert_eq!(value.accent, None);
        assert_eq!(value.design, None);
        assert_eq!(value.zoom, None);
        assert_eq!(value.notifications.position, Corner::Top);
        assert!(database.settings(SCOPE).unwrap().is_empty());
        assert!(store.view().changed.is_empty());
    }

    #[test]
    fn preferences_and_account_choices_survive_restart() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::open(dir.path()).unwrap();
        let mut store = PreferenceStore::open(database.clone()).unwrap();
        assert!(!store.get().mcp);
        assert!(!store.get().remote_images);
        assert!(!store.get().onboarded);
        assert!(!store.get().account_enabled("demo"));
        assert!(store.get().account_enabled("a1"));
        assert_eq!(store.get().list_density, ListDensity::Full);
        assert!(store.get().date_groups);
        assert!(store.get().prefetch.enabled);

        store
            .update(
                serde_json::from_value(serde_json::json!({
                    "mcp": true,
                    "account": "work",
                    "workspace": {"root": {}},
                    "remote_images": true,
                    "list_density": "compact",
                    "date_groups": false,
                    "notified_release": "0.4.0",
                    "font": "Segoe UI",
                    "font_size": 18,
                    "text_spacing": {"letter": 2, "word": 1, "line": 2},
                    "prefetch": {"enabled": false, "count": 10},
                    "accent": "#7AA2F7",
                    "notifications": {"enabled": true, "system": true, "position": "top-left"},
                    "accounts_enabled": {"demo": true, "a1": false}
                }))
                .unwrap(),
            )
            .unwrap();

        let mut restored = PreferenceStore::open(database).unwrap();
        let p = restored.get();
        assert!(p.mcp);
        assert_eq!(p.account.as_deref(), Some("work"));
        assert_eq!(p.workspace, Some(serde_json::json!({"root": {}})));
        assert!(p.remote_images);
        assert_eq!(p.list_density, ListDensity::Compact);
        assert!(!p.date_groups);
        assert_eq!(p.notified_release.as_deref(), Some("0.4.0"));
        assert_eq!(p.font.as_deref(), Some("Segoe UI"));
        assert_eq!(p.font_size, Some(18));
        assert_eq!(
            p.text_spacing,
            TextSpacing {
                letter: 2,
                word: 1,
                line: 2,
                paragraph: 0
            }
        );
        assert_eq!(
            p.prefetch,
            Prefetch {
                enabled: false,
                count: 10
            }
        );
        assert_eq!(p.accent.as_deref(), Some("#7aa2f7"));
        assert!(p.notifications.system);
        assert_eq!(p.notifications.position, Corner::TopLeft);
        assert!(p.account_enabled("demo"));
        assert!(!p.account_enabled("a1"));

        restored.clear_account("a1").unwrap();
        assert!(restored.get().account_enabled("a1"));
    }
}
