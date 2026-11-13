//! Typed bus surfaces of the bundled extensions (see "Bundled bus surfaces" in
//! `docs/extensions/README.md`). Each version is its own module, so a later `v2`
//! sits next to `v1` instead of replacing it.
//!
//! Requests the bundled side answers with `null` use `()` as their response, so
//! [`Context::request`](crate::Context::request) returns `Ok(None)` for them.

/// The message composer.
pub mod composer {
    pub mod v1 {
        use serde::{Deserialize, Serialize};

        /// The composer's current fields: the reply to [`GetDraft`], and the payload of a
        /// composer button press under `draft`.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Draft {
            pub recipient: String,
            pub subject: String,
            pub body: String,
        }

        /// Reads the open composer's draft; no answer when the composer is closed.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct GetDraft;
        crate::bus_request!(GetDraft => Draft, bus = "composer", version = 1, name = "get_draft");

        /// Replaces the fields that are set.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct SetDraft {
            #[serde(skip_serializing_if = "Option::is_none")]
            pub recipient: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub subject: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none")]
            pub body: Option<String>,
        }
        crate::bus_request!(SetDraft => (), bus = "composer", version = 1, name = "set_draft");

        /// Adds a button (at most four per extension, labels up to 32 characters). Pressing
        /// it sends `request` to your bus with `{"draft": Draft}`; reply with
        /// `{"draft": SetDraft}` to change the editor.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct AddButton {
            pub id: String,
            pub label: String,
            /// The versioned request name on your own bus, such as `v1/shout`.
            pub request: String,
        }
        crate::bus_request!(AddButton => (), bus = "composer", version = 1, name = "add_button");

        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct RemoveButton {
            pub id: String,
        }
        crate::bus_request!(RemoveButton => (), bus = "composer", version = 1, name = "remove_button");

        /// The composer opened.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct Opened {}
        crate::bus_event!(Opened, bus = "composer", version = 1, name = "opened");

        /// The owner pressed Send (informational).
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct Sending {}
        crate::bus_event!(Sending, bus = "composer", version = 1, name = "sending");
    }
}

/// The message reader.
pub mod reader {
    pub mod v1 {
        use serde::{Deserialize, Serialize};

        /// The open message's HTML; no answer for another message or a text-only one.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct GetHtml {
            pub message_id: String,
        }
        crate::bus_request!(GetHtml => String, bus = "reader", version = 1, name = "get_html");

        /// Replaces how the open message is shown. It stays in the sandboxed frame, and
        /// the reader names your extension as the one that changed it.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct SetHtml {
            pub message_id: String,
            pub html: String,
        }
        crate::bus_request!(SetHtml => (), bus = "reader", version = 1, name = "set_html");

        /// Shows a text banner above the open message (at most 280 characters; the latest
        /// three are kept).
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct AddBanner {
            pub message_id: String,
            pub text: String,
            pub tone: Tone,
        }
        crate::bus_request!(AddBanner => (), bus = "reader", version = 1, name = "add_banner");

        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all = "lowercase")]
        pub enum Tone {
            #[default]
            Info,
            Warning,
        }

        /// A message was opened. The reader waits for subscribers before showing the body.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct MessageOpened {
            pub message_id: String,
            pub subject: String,
            /// The sender's address, or their name when there is none.
            pub from: String,
            pub html: Option<String>,
            pub text: String,
        }
        crate::bus_event!(
            MessageOpened,
            bus = "reader",
            version = 1,
            name = "message_opened"
        );
    }
}

/// The message list.
pub mod inbox {
    pub mod v1 {
        use serde::{Deserialize, Serialize};

        /// The owner selected a message.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Selected {
            pub message_id: String,
        }
        crate::bus_event!(Selected, bus = "inbox", version = 1, name = "selected");
    }
}

/// Toast notifications.
pub mod notifications {
    pub mod v1 {
        use serde::{Deserialize, Serialize};

        /// Shows a toast, labelled with your extension's name.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(default)]
        pub struct Notify {
            pub title: String,
            pub body: String,
        }
        crate::bus_request!(Notify => (), bus = "notifications", version = 1, name = "notify");
    }
}
