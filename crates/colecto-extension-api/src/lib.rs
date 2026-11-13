//! Version 1 contracts shared by core services and the local Host.
use serde::{Deserialize, Serialize};

pub const API_VERSION: u32 = 1;
pub const DEMO_ACCOUNT: &str = "demo";

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum FolderRole {
    Inbox,
    Sent,
    Drafts,
    Archive,
    Trash,
    Junk,
    Other,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub role: FolderRole,
    pub unread: u32,
    pub total: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Message {
    #[serde(default)]
    pub seen: bool,
    #[serde(default)]
    pub flagged: bool,
    pub id: String,
    pub sender: String,
    pub address: String,
    pub subject: String,
    pub body: String,
    pub date: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    pub id: String,
    /// Comma-separated addresses, as typed in the composer.
    pub recipient: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cc: String,
    pub subject: String,
    pub body: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub in_reply_to: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub references: String,
    /// Re-attach this message's attachments when sending a forward.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forward_of: Option<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftInput {
    pub id: Option<String>,
    pub recipient: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cc: String,
    pub subject: String,
    pub body: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub in_reply_to: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub references: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub forward_of: Option<String>,
    /// Sent with the message only; drafts do not store uploaded files.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<OutgoingAttachment>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OutgoingAttachment {
    pub name: String,
    pub content_type: String,
    /// Standard base64 without a data-URL prefix.
    pub data: String,
}

/// Provider-fetched details that are not kept in mailbox snapshots.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MessageContent {
    /// Full decoded text, including messages too large for list snapshots.
    pub text: String,
    /// Untrusted HTML. Clients must render it only inside a script-free sandbox.
    pub html: Option<String>,
    pub attachments: Vec<AttachmentInfo>,
    pub message_id: String,
    pub references: String,
    pub reply_to: String,
    pub to: String,
    pub cc: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AttachmentInfo {
    pub index: usize,
    pub name: String,
    pub content_type: String,
    pub size: usize,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    Authentication,
    Forbidden,
    InvalidRequest,
    NotFound,
    Provider,
    ProviderAuthentication,
    ProviderTimeout,
    ProviderTls,
    ProviderRateLimited,
    Conflict,
    StorageOrService,
    DeliveryUncertain,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
    pub category: ErrorCategory,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    Imap,
    Jmap,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AccountSettings {
    #[serde(default)]
    pub name: String,
    pub address: String,
    pub protocol: Protocol,
    #[serde(default)]
    pub imap_host: String,
    #[serde(default = "imap_port")]
    pub imap_port: u16,
    #[serde(default)]
    pub smtp_host: String,
    #[serde(default = "smtp_port")]
    pub smtp_port: u16,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub session_url: String,
    /// IMAP upgrades a plain connection with STARTTLS instead of starting with TLS.
    // Optional fields are omitted when unset: the credential vault authenticates the
    // serialized settings, so existing records must serialize exactly as before.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub imap_starttls: bool,
    /// PEM certificate trusted instead of the system authorities, for IMAP and SMTP.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tls_certificate: String,
}
fn imap_port() -> u16 {
    993
}
fn smtp_port() -> u16 {
    587
}

#[derive(Deserialize)]
pub struct AccountInput {
    pub id: Option<String>,
    #[serde(flatten)]
    pub settings: AccountSettings,
    pub secret: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AccountInfo {
    pub id: String,
    pub settings: Option<AccountSettings>,
    pub connected: bool,
    pub configured: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SecurityInfo {
    pub cleanup_pending: bool,
    pub protected: bool,
    pub locked: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SecurityInput {
    pub action: String,
    #[serde(default)]
    pub password: String,
}

/// Service permissions are separate from native platform permissions.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Capability {
    #[serde(rename = "extensions.bus")]
    ExtensionsBus,
    #[serde(rename = "account.read")]
    AccountRead,
    #[serde(rename = "messages.list")]
    MessagesList,
    #[serde(rename = "messages.read")]
    MessagesRead,
    #[serde(rename = "drafts.list")]
    DraftsList,
    #[serde(rename = "drafts.read")]
    DraftsRead,
    #[serde(rename = "drafts.write")]
    DraftsWrite,
    #[serde(rename = "messages.send")]
    MessagesSend,
    #[serde(rename = "messages.label")]
    MessagesLabel,
    #[serde(rename = "messages.organize")]
    MessagesOrganize,
    #[serde(rename = "window.overlay")]
    WindowOverlay,
    /// Lets an API key move the owner's open Colecto windows to a page. Never granted to extensions.
    #[serde(rename = "app.navigate")]
    AppNavigate,
}

/// Pages that automation may open with `app.navigate`. The UI's navigator mirrors this list.
pub const NAVIGATION_TARGETS: &[&str] = &[
    "inbox",
    "drafts",
    "sent",
    "compose",
    "settings/accounts",
    "settings/mail",
    "settings/appearance",
    "settings/notifications",
    "settings/privacy",
    "settings/servers",
    "settings/extensions",
    "settings/ai",
    "settings/api-keys",
    "settings/send-approvals",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionManifest {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub settings: Vec<ExtensionSetting>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bus: Option<BusSubscriptions>,
    pub description: String,
    pub author: String,
    pub id: String,
    pub name: String,
    pub api_version: u32,
    pub slot: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub z_index: Option<u16>,
    pub capabilities: Vec<Capability>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionState {
    pub manifest: ExtensionManifest,
    pub bundled: bool,
    pub enabled: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionPolicyInput {
    pub enabled: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionInstallInput {
    /// A `.colectoext` package: a zip holding `manifest.toml` and `module.wasm`.
    pub package: Vec<u8>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionRunInput {
    pub action: String,
}

/// Declarative UI: all strings are rendered as text, never markup.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionView {
    pub title: String,
    pub text: String,
    pub actions: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LabelInput {
    pub name: String,
    pub message_id: Option<String>,
    #[serde(default)]
    pub remove: bool,
}

/// Events from other buses an extension receives, as `<bus>/<name>` (manifest `[bus]`).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BusSubscriptions {
    pub subscribe: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SettingOption {
    pub value: String,
    pub label: String,
}
/// The control an owner uses for one extension setting.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SettingKind {
    Toggle,
    Range,
    Choice,
    Text,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionSetting {
    pub key: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(rename = "type")]
    pub kind: SettingKind,
    pub default: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<SettingOption>>,
}
impl ExtensionSetting {
    pub fn accepts(&self, value: &serde_json::Value) -> bool {
        match self.kind {
            SettingKind::Toggle => value.is_boolean(),
            SettingKind::Text => value.is_string(),
            SettingKind::Choice => value.as_str().is_some_and(|v| {
                self.options
                    .as_ref()
                    .is_some_and(|o| o.iter().any(|o| o.value == v))
            }),
            SettingKind::Range => value.as_f64().is_some_and(|v| {
                v.is_finite()
                    && self.min.is_some_and(|min| v >= min)
                    && self.max.is_some_and(|max| v <= max)
            }),
        }
    }
}
pub fn validate_settings(settings: &[ExtensionSetting]) -> Result<(), String> {
    let mut keys = std::collections::HashSet::new();
    if settings.len() > 16 {
        return Err("At most 16 settings allowed".into());
    }
    for s in settings {
        let range = s.kind == SettingKind::Range;
        if s.key.is_empty()
            || s.key.len() > 32
            || !s
                .key
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
            || !keys.insert(&s.key)
            || s.label.is_empty()
            || s.label.len() > 64
            || s.description.as_ref().is_some_and(|d| d.len() > 256)
            || s.unit.as_ref().is_some_and(|u| u.len() > 8)
            || (!range
                && (s.min.is_some() || s.max.is_some() || s.step.is_some() || s.unit.is_some()))
            || (range
                && (!s.min.is_some_and(f64::is_finite)
                    || !s.max.is_some_and(f64::is_finite)
                    || s.min > s.max
                    || s.step.is_some_and(|n| !n.is_finite() || n <= 0.0)))
            || (s.kind != SettingKind::Choice && s.options.is_some())
            || (s.kind == SettingKind::Choice
                && !s.options.as_ref().is_some_and(|o| {
                    let mut values = std::collections::HashSet::new();
                    (2..=8).contains(&o.len())
                        && o.iter().all(|o| {
                            !o.value.is_empty() && !o.label.is_empty() && values.insert(&o.value)
                        })
                }))
            || !s.accepts(&s.default)
        {
            return Err(format!("Invalid extension setting: {}", s.key));
        }
    }
    Ok(())
}

#[cfg(test)]
mod setting_tests {
    use super::*;
    #[test]
    fn typed_settings_and_limits() {
        let range = serde_json::json!({"key":"speed","label":"Speed","type":"range","default":10,"min":2,"max":30});
        let parse = |v| serde_json::from_value::<ExtensionSetting>(v).unwrap();
        assert!(validate_settings(&[parse(range.clone())]).is_ok());
        for (kind, value) in [
            ("toggle", serde_json::json!(true)),
            ("text", serde_json::json!("hello")),
        ] {
            let s =
                parse(serde_json::json!({"key":"test","label":"Test","type":kind,"default":value}));
            assert!(validate_settings(&[s]).is_ok());
        }
        let choice = parse(
            serde_json::json!({"key":"test","label":"Test","type":"choice","default":"a","options":[{"value":"a","label":"A"},{"value":"b","label":"B"}]}),
        );
        assert!(validate_settings(&[choice]).is_ok());
        for (key, value) in [
            ("default", serde_json::json!(31)),
            ("default", serde_json::json!("10")),
            ("step", serde_json::json!(0)),
            ("key", serde_json::json!("Bad")),
            ("label", serde_json::json!("")),
            ("unit", serde_json::json!("123456789")),
        ] {
            let mut bad = range.clone();
            bad[key] = value;
            assert!(validate_settings(&[parse(bad)]).is_err());
        }
        assert!(validate_settings(&vec![parse(range.clone()); 17]).is_err());
        assert!(validate_settings(&[parse(range.clone()), parse(range)]).is_err());
    }
}

#[cfg(test)]
mod account_tests {
    use super::*;
    #[test]
    fn saved_settings_without_new_fields_serialize_unchanged() {
        // The credential vault authenticates this exact serialization, so a record
        // written before optional fields existed must round-trip byte for byte.
        let saved = r#"{"name":"Work","address":"a@example.test","protocol":"imap","imap_host":"imap.example.test","imap_port":993,"smtp_host":"smtp.example.test","smtp_port":587,"username":"a","session_url":""}"#;
        let settings: AccountSettings = serde_json::from_str(saved).unwrap();
        assert_eq!(serde_json::to_string(&settings).unwrap(), saved);
        let bridge = AccountSettings {
            imap_starttls: true,
            tls_certificate: "PEM".into(),
            ..settings
        };
        let json = serde_json::to_string(&bridge).unwrap();
        assert!(json.ends_with(r#""imap_starttls":true,"tls_certificate":"PEM"}"#));
        assert_eq!(
            serde_json::from_str::<AccountSettings>(&json).unwrap(),
            bridge
        );
    }
}
