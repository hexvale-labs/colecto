//! Guest-side copies of the Host's wire types.
//!
//! The Host's own types in `colecto-extension-api` reject unknown fields, which suits
//! input validation. These copies ignore unknown fields and default missing ones, so an
//! extension built today keeps working when a newer Host adds fields.

use serde::{Deserialize, Serialize};

/// A message from the selected account (`messages.list`, `messages.read`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Message {
    pub id: String,
    /// Display name of the sender.
    pub sender: String,
    /// Sender's email address.
    pub address: String,
    pub subject: String,
    pub body: String,
    pub date: String,
    pub seen: bool,
    pub flagged: bool,
}

/// A saved draft (`drafts.list`, `drafts.read`, `drafts.write`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Draft {
    pub id: String,
    /// Comma-separated addresses, as typed in the composer.
    pub recipient: String,
    pub cc: String,
    pub subject: String,
    pub body: String,
    pub in_reply_to: String,
    pub references: String,
    /// Re-attach this message's attachments when sending a forward.
    pub forward_of: Option<String>,
}

/// A draft to save or send. Leave `id` empty to create a new draft.
///
/// The Host validates this strictly, so only fields it knows are ever sent.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DraftInput {
    pub id: Option<String>,
    pub recipient: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub cc: String,
    pub subject: String,
    pub body: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub in_reply_to: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub references: String,
    /// Sending with this set also requires the `messages.read` capability.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forward_of: Option<String>,
    /// Sent with the message only; drafts do not store attachments.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub attachments: Vec<OutgoingAttachment>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct OutgoingAttachment {
    pub name: String,
    pub content_type: String,
    /// Standard base64 without a data-URL prefix.
    pub data: String,
}

/// Local labels after a change: label name to the message IDs that carry it.
pub type Labels = std::collections::BTreeMap<String, Vec<String>>;

#[cfg(test)]
mod tests {
    use super::*;
    use colecto_extension_api as api;

    // The Host serializes its own types; these must read them, and the Host must accept
    // what these send.
    #[test]
    fn matches_host_wire_types() {
        let host = api::Message {
            seen: true,
            flagged: false,
            id: "1".into(),
            sender: "Ada".into(),
            address: "ada@example.test".into(),
            subject: "Hi".into(),
            body: "Hello".into(),
            date: "2026-11-13".into(),
        };
        let guest: Message = serde_json::from_value(serde_json::to_value(&host).unwrap()).unwrap();
        assert_eq!(
            (guest.id.as_str(), guest.address.as_str(), guest.seen),
            ("1", "ada@example.test", true)
        );

        let host = api::Draft {
            id: "d".into(),
            recipient: "a@example.test".into(),
            cc: String::new(),
            subject: "S".into(),
            body: "B".into(),
            in_reply_to: String::new(),
            references: String::new(),
            forward_of: Some("1".into()),
        };
        let guest: Draft = serde_json::from_value(serde_json::to_value(&host).unwrap()).unwrap();
        assert_eq!(guest.forward_of.as_deref(), Some("1"));

        let input = DraftInput {
            recipient: "a@example.test".into(),
            attachments: vec![OutgoingAttachment {
                name: "a.txt".into(),
                content_type: "text/plain".into(),
                data: "aGk=".into(),
            }],
            ..Default::default()
        };
        let host: api::DraftInput =
            serde_json::from_value(serde_json::to_value(&input).unwrap()).unwrap();
        assert_eq!(host.attachments.len(), 1);
    }

    #[test]
    fn ignores_fields_from_newer_hosts() {
        let message: Message =
            serde_json::from_str(r#"{"id":"1","subject":"Hi","thread_id":"t","labels":["x"]}"#)
                .unwrap();
        assert_eq!((message.id.as_str(), message.subject.as_str()), ("1", "Hi"));
    }
}
