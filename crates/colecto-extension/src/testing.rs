//! Run extension code under `cargo test`, without Wasm or a running Colecto.
//!
//! ```ignore
//! use colecto_extension::testing::TestHost;
//! use serde_json::json;
//!
//! #[test]
//! fn shows_message_count() {
//!     let mut host = TestHost::new(|request| match request["operation"].as_str() {
//!         Some("messages.list") => Ok(json!([{"id": "1"}])),
//!         _ => Err("Capability not granted".into()),
//!     });
//!     let view = host.run(crate::colecto_main, "open").unwrap();
//!     assert_eq!(view["text"], "1 messages");
//! }
//! ```
//!
//! Only available on non-Wasm targets.

use crate::Context;
use colecto_extension_sys::mock;
use serde_json::{Value, json};
use std::cell::RefCell;
use std::rc::Rc;

type Service = Box<dyn FnMut(&Value) -> Result<Value, String>>;

/// A stand-in Host that answers service requests with a function you provide.
pub struct TestHost {
    service: Rc<RefCell<Service>>,
}

impl TestHost {
    /// `service` receives each request (such as `{"operation":"messages.list"}`) and
    /// returns its `data`, or the error the Host would report.
    pub fn new(service: impl FnMut(&Value) -> Result<Value, String> + 'static) -> Self {
        Self {
            service: Rc::new(RefCell::new(Box::new(service))),
        }
    }

    /// Runs one invocation of `entry` (normally `crate::colecto_main`, defined by
    /// `export!`) with the given action, and returns what it emitted.
    pub fn run(&mut self, entry: fn(), action: &str) -> Option<Value> {
        self.begin(action);
        entry();
        mock::finish().and_then(|bytes| serde_json::from_slice(&bytes).ok())
    }

    /// Calls `f` with a [`Context`] backed by this host, for testing one handler directly.
    pub fn with<T>(&mut self, f: impl FnOnce(&mut Context) -> T) -> T {
        self.begin("");
        let result = f(&mut Context::new(None));
        mock::finish();
        result
    }

    fn begin(&mut self, action: &str) {
        let service = Rc::clone(&self.service);
        mock::begin(
            action.as_bytes(),
            Box::new(move |input| {
                let request: Value = serde_json::from_slice(input).ok()?;
                let response = match (service.borrow_mut())(&request) {
                    Ok(data) => json!({"ok": true, "data": data}),
                    Err(error) => json!({"ok": false, "error": error}),
                };
                serde_json::to_vec(&response).ok()
            }),
        );
    }
}
