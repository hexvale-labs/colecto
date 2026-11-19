//! Settings that API keys may read and change over MCP (`settings.read`, `settings.update`).
//!
//! Each area needs its own key capability: `settings.appearance`, `settings.notifications`
//! and `settings.extensions` (extensions' own pages). Credentials, the server connection,
//! background services, API keys, MCP itself, extension installation, send approvals,
//! accounts and the workspace layout stay owner only: nothing here can reach them.
use crate::{Failure, Host, api_keys, forbidden, preferences};
use colecto_extension_api::{Capability, ExtensionSetting, SettingSection};
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;

/// What `settings.update` may change. Present fields replace the stored ones.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SettingsInput {
    appearance: Option<AppearanceInput>,
    /// Merged into the stored notification preferences, field by field ([`NOTIFICATION_FIELDS`]).
    notifications: Option<Map<String, Value>>,
    /// Extension id → setting key → value, checked against the extension's schema.
    #[serde(default)]
    extensions: BTreeMap<String, Map<String, Value>>,
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct AppearanceInput {
    /// `#rrggbb`, or an empty string for the default accent.
    accent: Option<String>,
    font: Option<String>,
    font_size: Option<u8>,
    /// Merged into the stored steps, field by field.
    text_spacing: Option<Map<String, Value>>,
}

const NOTIFICATION_FIELDS: &[&str] = &["enabled", "system", "position"];

/// The key capability an extension setting needs: its core section, or its extension's page.
fn area(setting: &ExtensionSetting) -> Capability {
    match setting.section {
        Some(SettingSection::Appearance) => Capability::SettingsAppearance,
        Some(SettingSection::Notifications) => Capability::SettingsNotifications,
        None => Capability::SettingsExtensions,
    }
}

fn granted(host: &Host, key: &str, account: &str) -> Result<Vec<Capability>, Failure> {
    let mut keys = host.keys.lock().map_err(|_| forbidden())?;
    let now = api_keys::now();
    Ok([
        Capability::SettingsAppearance,
        Capability::SettingsNotifications,
        Capability::SettingsExtensions,
    ]
    .into_iter()
    .filter(|&capability| keys.allows(key, account, capability, now))
    .collect())
}

fn schema(host: &Host, id: &str) -> Result<(Vec<ExtensionSetting>, Map<String, Value>), Failure> {
    let view = host
        .extensions
        .lock()
        .map_err(|_| forbidden())?
        .settings(id)
        .map_err(|e| Failure(axum::http::StatusCode::BAD_REQUEST, e))?;
    let schema = serde_json::from_value(view["schema"].clone()).unwrap_or_default();
    let values = view["values"].as_object().cloned().unwrap_or_default();
    Ok((schema, values))
}

/// The settings this key may see, by area. Areas it was not granted are left out.
pub(crate) fn read(host: &Host, key: &str, account: &str) -> Result<Value, Failure> {
    let areas = granted(host, key, account)?;
    if areas.is_empty() {
        return Err(forbidden());
    }
    let saved = host
        .preferences
        .lock()
        .map_err(|_| forbidden())?
        .get()
        .clone();
    let mut result = Map::new();
    if areas.contains(&Capability::SettingsAppearance) {
        result.insert(
            "appearance".into(),
            json!({"accent": saved.accent, "font": saved.font, "font_size": saved.font_size, "text_spacing": saved.text_spacing}),
        );
    }
    if areas.contains(&Capability::SettingsNotifications) {
        let notifications = &saved.notifications;
        result.insert(
            "notifications".into(),
            json!({"enabled": notifications.enabled, "system": notifications.system, "position": notifications.position}),
        );
    }
    let states = host.extensions.lock().map_err(|_| forbidden())?.list("");
    let mut extensions = Vec::new();
    for state in states.iter().filter(|s| s.discontinued.is_none()) {
        let (schema, values) = schema(host, &state.manifest.id)?;
        let settings: Vec<Value> = schema
            .into_iter()
            .filter(|setting| areas.contains(&area(setting)))
            .map(|setting| {
                let mut entry = serde_json::to_value(&setting).unwrap_or_default();
                entry["value"] = values.get(&setting.key).cloned().unwrap_or_default();
                entry
            })
            .collect();
        if !settings.is_empty() {
            extensions.push(json!({"id": state.manifest.id, "name": state.manifest.name, "enabled": state.enabled, "settings": settings}));
        }
    }
    result.insert("extensions".into(), Value::Array(extensions));
    Ok(Value::Object(result))
}

/// Check every change against the key's grants and the same limits Settings uses, then save
/// them and tell open windows. Nothing is saved when any part is refused or invalid.
pub(crate) fn update(
    host: &Host,
    key: &str,
    account: &str,
    input: SettingsInput,
) -> Result<Value, Failure> {
    let bad = |e: String| Failure(axum::http::StatusCode::BAD_REQUEST, e);
    let areas = granted(host, key, account)?;
    let needs = |capability| {
        areas
            .contains(&capability)
            .then_some(())
            .ok_or_else(forbidden)
    };
    if input.appearance.is_some() {
        needs(Capability::SettingsAppearance)?;
    }
    if input.notifications.is_some() {
        needs(Capability::SettingsNotifications)?;
    }
    for (id, values) in &input.extensions {
        let (schema, _) = schema(host, id)?;
        for setting_key in values.keys() {
            let setting = schema
                .iter()
                .find(|s| s.key == *setting_key)
                .ok_or_else(|| bad(format!("Invalid setting: {setting_key}")))?;
            needs(area(setting))?;
        }
        host.extensions
            .lock()
            .map_err(|_| forbidden())?
            .check_settings(id, values)
            .map_err(bad)?;
    }
    let mut changes = preferences::PreferencesInput::default();
    if let Some(appearance) = input.appearance {
        changes.accent = appearance.accent;
        changes.font = appearance.font;
        changes.font_size = appearance.font_size;
        if let Some(fields) = appearance.text_spacing {
            let stored = host
                .preferences
                .lock()
                .map_err(|_| forbidden())?
                .get()
                .text_spacing;
            let mut merged = json!(stored);
            for (field, value) in fields {
                merged[field] = value;
            }
            // Unknown fields and values outside 0–255 fail here; the store checks the steps.
            changes.text_spacing = Some(
                serde_json::from_value(merged).map_err(|_| bad("Invalid text spacing".into()))?,
            );
        }
    }
    let preferences_changed = changes.accent.is_some()
        || changes.font.is_some()
        || changes.font_size.is_some()
        || changes.text_spacing.is_some()
        || input.notifications.is_some();
    if preferences_changed {
        let mut store = host.preferences.lock().map_err(|_| forbidden())?;
        if let Some(fields) = input.notifications {
            let mut merged = serde_json::to_value(&store.get().notifications)
                .map_err(|_| bad("Invalid notification settings".into()))?;
            for (field, value) in fields {
                // What to be notified about lives in the Notifications extension's settings;
                // the older mail, approvals and updates fields here no longer do anything.
                if !NOTIFICATION_FIELDS.contains(&field.as_str()) {
                    return Err(bad(format!("Invalid notification setting: {field}")));
                }
                merged[field] = value;
            }
            changes.notifications = Some(
                serde_json::from_value(merged)
                    .map_err(|_| bad("Invalid notification settings".into()))?,
            );
        }
        store.update(changes).map_err(bad)?;
        let _ = host.events.send(("preferences", "{}".into()));
    }
    for (id, values) in input.extensions {
        if values.is_empty() {
            continue;
        }
        host.extensions
            .lock()
            .map_err(|_| forbidden())?
            .update_settings(&id, values)
            .map_err(bad)?;
        let _ = host
            .events
            .send(("extension.settings", json!({"id": id}).to_string()));
    }
    read(host, key, account)
}
