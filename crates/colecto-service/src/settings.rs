//! Settings that API keys may read and change over MCP (`settings.read`, `settings.update`).
//!
//! Each area needs its own key capability: `settings.appearance`, `settings.notifications`,
//! and `settings.mail`. Credentials, server connections, background services, API keys,
//! MCP itself, send approvals, and accounts stay owner-only: nothing here can reach them.
use crate::api_keys::{self, Principal};
use crate::error::ServiceError;
use crate::events::ServiceEvent;
use crate::preferences::{ListDensity, PreferencesInput, Prefetch};
use crate::{ServiceInner, authorize_inner};
use colecto_core::Capability;
use serde::Deserialize;
use serde_json::{Map, Value, json};

/// What `settings.update` may change. Present fields replace the stored ones.
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsInput {
    pub appearance: Option<AppearanceInput>,
    /// Merged into the stored notification preferences, field by field ([`NOTIFICATION_FIELDS`]).
    pub notifications: Option<Map<String, Value>>,
    pub mail: Option<MailInput>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AppearanceInput {
    /// `#rrggbb`, or an empty string for the default accent.
    pub accent: Option<String>,
    pub font: Option<String>,
    pub font_size: Option<u8>,
    /// Merged into the stored steps, field by field.
    pub text_spacing: Option<Map<String, Value>>,
    pub list_density: Option<ListDensity>,
    pub date_groups: Option<bool>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MailInput {
    pub remote_images: Option<bool>,
    /// Merged into the stored prefetch settings, field by field (`enabled`, `count`).
    pub prefetch: Option<Map<String, Value>>,
}

const NOTIFICATION_FIELDS: &[&str] = &["enabled", "system", "position"];
const PREFETCH_FIELDS: &[&str] = &["enabled", "count"];

fn granted(
    inner: &ServiceInner,
    principal: &Principal,
    account: &str,
) -> Result<Vec<Capability>, ServiceError> {
    let caps = [
        Capability::SettingsAppearance,
        Capability::SettingsNotifications,
        Capability::SettingsMail,
    ];
    match principal {
        Principal::Owner => {
            if !inner
                .accounts
                .lock()
                .map_err(|_| ServiceError::forbidden())?
                .list()
                .iter()
                .any(|a| a.id == account)
            {
                return Err(ServiceError::forbidden());
            }
            Ok(caps.into_iter().collect())
        }
        Principal::ApiKey(key) => {
            let mut keys = inner.keys.lock().map_err(|_| ServiceError::forbidden())?;
            let now = api_keys::now();
            Ok(caps
                .into_iter()
                .filter(|&capability| keys.allows(key, account, capability, now))
                .collect())
        }
    }
}

/// The settings this principal may see, by area. Areas it was not granted are left out.
pub(crate) fn read(
    inner: &ServiceInner,
    principal: &Principal,
    account: &str,
) -> Result<Value, ServiceError> {
    let areas = granted(inner, principal, account)?;
    if areas.is_empty() {
        return Err(ServiceError::forbidden());
    }
    let saved = inner
        .preferences
        .lock()
        .map_err(|_| ServiceError::forbidden())?
        .get()
        .clone();
    let mut result = Map::new();
    if areas.contains(&Capability::SettingsAppearance) {
        result.insert(
            "appearance".into(),
            json!({
                "accent": saved.accent,
                "font": saved.font,
                "font_size": saved.font_size,
                "text_spacing": saved.text_spacing,
                "list_density": saved.list_density,
                "date_groups": saved.date_groups,
            }),
        );
    }
    if areas.contains(&Capability::SettingsNotifications) {
        let notifications = &saved.notifications;
        result.insert(
            "notifications".into(),
            json!({
                "enabled": notifications.enabled,
                "system": notifications.system,
                "position": notifications.position,
            }),
        );
    }
    if areas.contains(&Capability::SettingsMail) {
        result.insert(
            "mail".into(),
            json!({
                "remote_images": saved.remote_images,
                "prefetch": saved.prefetch,
            }),
        );
    }
    Ok(Value::Object(result))
}

/// Check every change against the caller's grants and the same limits Settings uses, then save
/// them and notify listeners. Nothing is saved when any part is refused or invalid.
pub(crate) fn update(
    inner: &ServiceInner,
    principal: &Principal,
    account: &str,
    input: SettingsInput,
) -> Result<Value, ServiceError> {
    let areas = granted(inner, principal, account)?;
    let needs = |capability| {
        areas
            .contains(&capability)
            .then_some(())
            .ok_or_else(ServiceError::forbidden)
    };
    if input.appearance.is_some() {
        needs(Capability::SettingsAppearance)?;
        authorize_inner(inner, principal, account, Capability::SettingsAppearance)?;
    }
    if input.notifications.is_some() {
        needs(Capability::SettingsNotifications)?;
        authorize_inner(inner, principal, account, Capability::SettingsNotifications)?;
    }
    if input.mail.is_some() {
        needs(Capability::SettingsMail)?;
        authorize_inner(inner, principal, account, Capability::SettingsMail)?;
    }
    let mut changes = PreferencesInput::default();
    let stored = inner
        .preferences
        .lock()
        .map_err(|_| ServiceError::forbidden())?
        .get()
        .clone();
    if let Some(appearance) = input.appearance {
        changes.accent = appearance.accent;
        changes.font = appearance.font;
        changes.font_size = appearance.font_size;
        changes.list_density = appearance.list_density;
        changes.date_groups = appearance.date_groups;
        if let Some(fields) = appearance.text_spacing {
            let mut merged = json!(stored.text_spacing);
            for (field, value) in fields {
                merged[field] = value;
            }
            // Unknown fields and values outside 0..=255 fail here; the store checks the steps.
            changes.text_spacing = Some(
                serde_json::from_value(merged)
                    .map_err(|_| ServiceError::bad_request("Invalid text spacing"))?,
            );
        }
    }
    if let Some(fields) = input.notifications {
        let mut merged = serde_json::to_value(&stored.notifications)
            .map_err(|_| ServiceError::bad_request("Invalid notification settings"))?;
        for (field, value) in fields {
            if !NOTIFICATION_FIELDS.contains(&field.as_str()) {
                return Err(ServiceError::bad_request(format!(
                    "Invalid notification setting: {field}"
                )));
            }
            merged[field] = value;
        }
        changes.notifications = Some(
            serde_json::from_value(merged)
                .map_err(|_| ServiceError::bad_request("Invalid notification settings"))?,
        );
    }
    if let Some(mail) = input.mail {
        changes.remote_images = mail.remote_images;
        if let Some(fields) = mail.prefetch {
            let mut merged = serde_json::to_value(&stored.prefetch)
                .map_err(|_| ServiceError::bad_request("Invalid prefetch settings"))?;
            for (field, value) in fields {
                if !PREFETCH_FIELDS.contains(&field.as_str()) {
                    return Err(ServiceError::bad_request(format!(
                        "Invalid prefetch setting: {field}"
                    )));
                }
                merged[field] = value;
            }
            let parsed: Prefetch = serde_json::from_value(merged)
                .map_err(|_| ServiceError::bad_request("Invalid prefetch settings"))?;
            changes.prefetch = Some(parsed);
        }
    }
    let changed = changes.accent.is_some()
        || changes.font.is_some()
        || changes.font_size.is_some()
        || changes.text_spacing.is_some()
        || changes.list_density.is_some()
        || changes.date_groups.is_some()
        || changes.notifications.is_some()
        || changes.remote_images.is_some()
        || changes.prefetch.is_some();
    if changed {
        inner
            .preferences
            .lock()
            .map_err(|_| ServiceError::forbidden())?
            .update(changes)
            .map_err(ServiceError::bad_request)?;
        let _ = inner.events.send(ServiceEvent::Preferences);
    }
    read(inner, principal, account)
}
