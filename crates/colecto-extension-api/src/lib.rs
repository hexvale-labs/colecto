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
