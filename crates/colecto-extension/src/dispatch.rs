//! Routes one invocation's action to the function registered with `export!`.

use crate::{BusEvent, BusRequest, Context, MailEvent, Result};
use colecto_extension_sys as sys;
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
    Mail(MailEvent),
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
    #[serde(rename = "mail")]
    Mail { account: String, ids: Vec<String> },
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
            Ok(Envelope::Mail { account, ids }) => Action::Mail(MailEvent { account, ids }),
            Err(_) => Action::Done,
        };
        Self { action }
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

    pub fn mail<F>(&mut self, handler: F)
    where
        F: FnOnce(MailEvent, &mut Context) -> Result<()>,
    {
        let Action::Mail(_) = &self.action else {
            return;
        };
        let Action::Mail(event) = std::mem::replace(&mut self.action, Action::Done) else {
            unreachable!()
        };
        // The Host ignores output for mail events, so errors have nowhere to go.
        let _ = handler(event, &mut Context::new(None));
    }

    /// Nothing matched; requests and events then go unanswered, which the Host allows.
    pub fn finish(self) {}
}

fn emit(value: &impl Serialize) {
    if let Ok(bytes) = serde_json::to_vec(value) {
        sys::emit(&bytes);
    }
}
