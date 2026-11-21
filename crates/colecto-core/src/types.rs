//! Domain and wire types for Colecto.
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
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
    /// The conversation this message belongs to: JMAP's `threadId`, or for IMAP the
    /// first `References` id, else `In-Reply-To`, else its own `Message-ID`. Empty when unknown.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub thread: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OutgoingAttachment {
    pub name: String,
    pub content_type: String,
    /// Standard base64 without a data-URL prefix.
    pub data: String,
}

/// Provider-fetched details that are not kept in mailbox snapshots.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MessageContent {
    /// Full decoded text, including messages too large for list snapshots.
    pub text: String,
    /// Sanitized HTML ready for sandboxed rendering.
    pub html: Option<String>,
    /// Number of remote images stripped during sanitization when remote images are blocked.
    #[serde(default)]
    pub blocked_remote_images: usize,
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

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct AccountInput {
    pub id: Option<String>,
    #[serde(flatten)]
    pub settings: AccountSettings,
    pub secret: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccountInfo {
    pub id: String,
    pub settings: Option<AccountSettings>,
    pub connected: bool,
    pub configured: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityInfo {
    pub cleanup_pending: bool,
    pub protected: bool,
    pub locked: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SecurityInput {
    pub action: String,
    #[serde(default)]
    pub password: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LabelInput {
    pub name: String,
    pub message_id: Option<String>,
    #[serde(default)]
    pub remove: bool,
}

/// First-party service permissions for scoped API keys and automation.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum Capability {
    #[serde(rename = "account.read")]
    AccountRead,
    #[serde(rename = "messages.list")]
    MessagesList,
    #[serde(rename = "messages.read")]
    MessagesRead,
    #[serde(rename = "messages.organize")]
    MessagesOrganize,
    #[serde(rename = "messages.label")]
    MessagesLabel,
    #[serde(rename = "messages.send")]
    MessagesSend,
    #[serde(rename = "drafts.list")]
    DraftsList,
    #[serde(rename = "drafts.read")]
    DraftsRead,
    #[serde(rename = "drafts.write")]
    DraftsWrite,
    #[serde(rename = "ai.text")]
    AiText,
    #[serde(rename = "ai.decide")]
    AiDecide,
    #[serde(rename = "ai.configure")]
    AiConfigure,
    #[serde(rename = "releases.read")]
    ReleasesRead,
    /// Lets an API key move open Colecto windows to a page.
    #[serde(rename = "app.navigate")]
    AppNavigate,
    /// Lets an API key read and change Settings -> Appearance.
    #[serde(rename = "settings.appearance")]
    SettingsAppearance,
    /// Lets an API key read and change Settings -> Notifications.
    #[serde(rename = "settings.notifications")]
    SettingsNotifications,
    /// Lets an API key read and change Settings -> Mail (prefetch, remote images, etc.).
    #[serde(rename = "settings.mail")]
    SettingsMail,
}

/// Pages that automation may open with `app.navigate`.
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
    "settings/ai",
    "settings/api-keys",
    "settings/send-approvals",
];

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
