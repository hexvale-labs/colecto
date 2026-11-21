use crate::sync::AccountSync;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "event", content = "data", rename_all = "snake_case")]
pub enum ServiceEvent {
    Mail {
        account: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        ids: Vec<String>,
    },
    Sync(AccountSync),
    Approval {
        id: String,
        account: String,
        from: String,
        subject: String,
    },
    Navigate {
        target: String,
        account: String,
    },
    Mcp {
        on: bool,
    },
    Preferences,
    LabelsUpdated {
        account: String,
    },
    Shutdown,
}

impl ServiceEvent {
    pub fn event_name(&self) -> &'static str {
        match self {
            Self::Mail { .. } => "mail",
            Self::Sync(_) => "sync",
            Self::Approval { .. } => "approval",
            Self::Navigate { .. } => "navigate",
            Self::Mcp { .. } => "mcp",
            Self::Preferences => "preferences",
            Self::LabelsUpdated { .. } => "labels",
            Self::Shutdown => "",
        }
    }

    pub fn payload_json(&self, open_windows: Option<usize>) -> String {
        match self {
            Self::Mail { account, ids } => {
                let mut value = if ids.is_empty() {
                    serde_json::json!({ "account": account })
                } else {
                    serde_json::json!({ "account": account, "ids": ids })
                };
                if let Some(windows) = open_windows {
                    value["windows"] = windows.into();
                }
                value.to_string()
            }
            Self::Sync(sync) => serde_json::to_string(sync).unwrap_or_default(),
            Self::Approval {
                id,
                account,
                from,
                subject,
            } => serde_json::json!({
                "id": id,
                "account": account,
                "from": from,
                "subject": subject,
            })
            .to_string(),
            Self::Navigate { target, account } => serde_json::json!({
                "target": target,
                "account": account,
            })
            .to_string(),
            Self::Mcp { on } => serde_json::json!({ "on": on }).to_string(),
            Self::Preferences => "{}".to_owned(),
            Self::LabelsUpdated { account } => {
                serde_json::json!({ "account": account }).to_string()
            }
            Self::Shutdown => String::new(),
        }
    }
}
