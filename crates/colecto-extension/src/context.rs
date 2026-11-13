use crate::{BusEvent, BusRequest, Draft, DraftInput, Error, Labels, Message, Result};
use colecto_extension_sys as sys;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Value, json};

/// Access to the Host during one invocation.
///
/// Every call is checked against the extension's capabilities and acts on the account
/// the owner has selected. An invocation may make at most eight calls; later ones fail
/// with [`Error::Bridge`].
pub struct Context {
    sender: Option<String>,
}

impl Context {
    pub(crate) fn new(sender: Option<String>) -> Self {
        Self { sender }
    }

    /// While answering a bus request: the ID of the extension that sent it.
    pub fn sender(&self) -> Option<&str> {
        self.sender.as_deref()
    }

    /// The owner's values for the manifest's `[[settings]]`, merged over defaults.
    /// Needs no capability.
    pub fn settings<S: DeserializeOwned>(&mut self) -> Result<S> {
        self.call(&json!({"operation": "settings.read"}))
    }

    /// Whether the selected account is connected (`account.read`).
    pub fn account_connected(&mut self) -> Result<bool> {
        self.call(&json!({"operation": "account.read"}))
    }

    /// The selected account's inbox (`messages.list`).
    pub fn messages(&mut self) -> Result<Vec<Message>> {
        self.call(&json!({"operation": "messages.list"}))
    }

    /// One message (`messages.read`).
    pub fn message(&mut self, id: &str) -> Result<Message> {
        self.call(&json!({"operation": "messages.read", "id": id}))
    }

    /// Saved drafts (`drafts.list`).
    pub fn drafts(&mut self) -> Result<Vec<Draft>> {
        self.call(&json!({"operation": "drafts.list"}))
    }

    /// One draft (`drafts.read`).
    pub fn draft(&mut self, id: &str) -> Result<Draft> {
        self.call(&json!({"operation": "drafts.read", "id": id}))
    }

    /// Creates or updates a draft (`drafts.write`).
    pub fn save_draft(&mut self, draft: &DraftInput) -> Result<Draft> {
        self.call(&json!({"operation": "drafts.write", "draft": draft}))
    }

    /// Sends a message (`messages.send`). The owner may have to approve it first.
    pub fn send(&mut self, draft: &DraftInput) -> Result<()> {
        self.call::<Value>(&json!({"operation": "messages.send", "draft": draft}))
            .map(drop)
    }

    /// Adds or, with `remove`, removes a local label; without a message it creates or
    /// deletes the label itself (`messages.label`).
    pub fn label(&mut self, name: &str, message_id: Option<&str>, remove: bool) -> Result<Labels> {
        self.call(&json!({
            "operation": "messages.label",
            "label": {"name": name, "message_id": message_id, "remove": remove},
        }))
    }

    /// Sends a request to another extension's bus (`extensions.bus`).
    ///
    /// `Ok(None)` means there was no answer: the bus is missing or disabled, nothing
    /// handled the request, or the handler answered `null`. The Host does not say which.
    pub fn request<R: BusRequest>(&mut self, request: &R) -> Result<Option<R::Response>> {
        self.call(&json!({
            "operation": "bus.request",
            "bus": R::BUS,
            "name": R::NAME,
            "payload": request,
        }))
    }

    /// Publishes an event on this extension's own bus (`extensions.bus`). The Host
    /// always uses the caller's bus, so `E::BUS` should be this extension's ID.
    pub fn emit<E: BusEvent>(&mut self, event: &E) -> Result<()> {
        self.call::<Value>(&json!({"operation": "bus.emit", "name": E::NAME, "payload": event}))
            .map(drop)
    }

    /// Sends a raw service request and returns its `data`. Use it for operations this
    /// SDK does not wrap yet.
    pub fn call<T: DeserializeOwned>(&mut self, request: &impl Serialize) -> Result<T> {
        let input = serde_json::to_vec(request)?;
        let output = sys::request(&input).ok_or(Error::Bridge)?;
        let mut envelope: Value = serde_json::from_slice(&output)?;
        if envelope["ok"] != Value::Bool(true) {
            let error = envelope["error"]
                .as_str()
                .unwrap_or("Service request failed");
            return Err(Error::Service(error.to_owned()));
        }
        let data = envelope
            .get_mut("data")
            .map(Value::take)
            .unwrap_or_default();
        Ok(serde_json::from_value(data)?)
    }
}
