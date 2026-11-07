//! Version 1 contracts shared by core services and the local demo Host.
use serde::{Deserialize, Serialize};

pub const API_VERSION: u32 = 1;
pub const DEMO_ACCOUNT: &str = "demo";

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
    pub recipient: String,
    pub subject: String,
    pub body: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DraftInput {
    pub id: Option<String>,
    pub recipient: String,
    pub subject: String,
    pub body: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ApiError {
    pub error: String,
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
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionManifest {
    pub description: String,
    pub author: String,
    pub id: String,
    pub name: String,
    pub api_version: u32,
    pub slot: String,
    pub capabilities: Vec<Capability>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionState {
    pub manifest: ExtensionManifest,
    pub bundled: bool,
    pub enabled: bool,
    pub grants: Vec<Capability>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionPolicyInput {
    pub enabled: bool,
    pub grants: Vec<Capability>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExtensionInstallInput {
    pub manifest: String,
    pub module: Vec<u8>,
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
