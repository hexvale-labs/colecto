//! Routes one invocation's action to the function registered with `export!`.

use crate::{
    BusEvent, BusRequest, Context, Frame, Overlay, PanelEvent, Pointer, Result, View, Viewport,
};
use colecto_extension_sys as sys;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

enum Action {
    /// Already handled, or nothing to do.
    Done,
    BusRequest {
        from: String,
        name: String,
        payload: Value,
    },
    BusEvent {
        bus: String,
        name: String,
        payload: Value,
    },
    /// A panel action (plain text) or an overlay frame (JSON), depending on the slot.
    Slot(String),
}

/// The action shapes from `docs/extensions/README.md`; unknown fields are ignored.
#[derive(Deserialize)]
#[serde(tag = "event")]
enum Envelope {
    #[serde(rename = "bus.request")]
    BusRequest {
        from: String,
        name: String,
        #[serde(default)]
        payload: Value,
    },
    #[serde(rename = "bus.event")]
    BusEvent {
        bus: String,
        name: String,
        #[serde(default)]
        payload: Value,
    },
}

#[derive(Deserialize)]
struct RawFrame {
    #[serde(default)]
    time_ms: f64,
    #[serde(default)]
    pointer: Pointer,
    #[serde(default)]
    viewport: Viewport,
    #[serde(default)]
    state: String,
}

pub struct Dispatch {
    action: Action,
}

impl Dispatch {
    pub fn begin() -> Self {
        let Some(bytes) = sys::action() else {
            return Self {
                action: Action::Done,
            };
        };
        let action = match serde_json::from_slice::<Envelope>(&bytes) {
            Ok(Envelope::BusRequest {
                from,
                name,
                payload,
            }) => Action::BusRequest {
                from,
                name,
                payload,
            },
            Ok(Envelope::BusEvent { bus, name, payload }) => {
                Action::BusEvent { bus, name, payload }
            }
            Err(_) => match String::from_utf8(bytes) {
                Ok(text) => Action::Slot(text),
                Err(_) => Action::Done,
            },
        };
        Self { action }
    }

    pub fn panel<F>(&mut self, handler: F)
    where
        F: FnOnce(PanelEvent, &mut Context) -> Result<View>,
    {
        let Some(text) = self.take_slot() else { return };
        let event = if text == "open" {
            PanelEvent::Open
        } else {
            PanelEvent::Action(text)
        };
        let view = handler(event, &mut Context::new(None))
            .unwrap_or_else(|error| View::new("Error", error.to_string()));
        if !emit(&view) {
            // A rejected emit leaves the slot open; explain instead of a generic Host error.
            emit(&View::new(
                "Error",
                "The extension produced a view outside the limits (title 256 bytes, text 16 KiB, 16 actions of 1–128 bytes).",
            ));
        }
    }

    pub fn overlay<S, F>(&mut self, handler: F)
    where
        S: Serialize + DeserializeOwned + Default,
        F: FnOnce(Frame<S>, &mut Context) -> Result<Overlay<S>>,
    {
        let Some(text) = self.take_slot() else { return };
        let Ok(raw) = serde_json::from_str::<RawFrame>(&text) else {
            return;
        };
        let frame = Frame {
            time_ms: raw.time_ms,
            pointer: raw.pointer,
            viewport: raw.viewport,
            state: serde_json::from_str(&raw.state).unwrap_or_default(),
        };
        // An error emits nothing; the Host then reports it and stops the overlay.
        let Ok(overlay) = handler(frame, &mut Context::new(None)) else {
            return;
        };
        let Ok(state) = serde_json::to_string(&overlay.state) else {
            return;
        };
        emit(&json!({"overlay": {"items": overlay.items}, "state": state}));
    }

    pub fn request<R, F>(&mut self, handler: F)
    where
        R: BusRequest,
        F: FnOnce(R, &mut Context) -> Result<R::Response>,
    {
        let Action::BusRequest { name, .. } = &self.action else {
            return;
        };
        if name != R::NAME {
            return;
        }
        let Action::BusRequest { from, payload, .. } =
            std::mem::replace(&mut self.action, Action::Done)
        else {
            unreachable!()
        };
        // A payload of the wrong shape or an error means no answer; the caller sees null.
        let Ok(request) = serde_json::from_value::<R>(payload) else {
            return;
        };
        if let Ok(response) = handler(request, &mut Context::new(Some(from))) {
            emit(&json!({"bus": {"response": response}}));
        }
    }

    pub fn event<E, F>(&mut self, handler: F)
    where
        E: BusEvent,
        F: FnOnce(E, &mut Context) -> Result<()>,
    {
        let Action::BusEvent { bus, name, .. } = &self.action else {
            return;
        };
        if bus != E::BUS || name != E::NAME {
            return;
        }
        let Action::BusEvent { payload, .. } = std::mem::replace(&mut self.action, Action::Done)
        else {
            unreachable!()
        };
        // The Host ignores event output, so errors have nowhere to go.
        if let Ok(event) = serde_json::from_value::<E>(payload) {
            let _ = handler(event, &mut Context::new(None));
        }
    }

    /// Nothing matched; requests and events then go unanswered, which the Host allows.
    pub fn finish(self) {}

    fn take_slot(&mut self) -> Option<String> {
        if !matches!(self.action, Action::Slot(_)) {
            return None;
        }
        match std::mem::replace(&mut self.action, Action::Done) {
            Action::Slot(text) => Some(text),
            _ => None,
        }
    }
}

fn emit(value: &impl Serialize) -> bool {
    serde_json::to_vec(value).is_ok_and(|bytes| sys::emit(&bytes))
}
