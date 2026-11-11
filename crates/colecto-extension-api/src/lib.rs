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
    "settings/appearance",
    "settings/notifications",
    "settings/privacy",
    "settings/servers",
    "settings/extensions",
    "settings/api-keys",
    "settings/send-approvals",
];

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionManifest {
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
