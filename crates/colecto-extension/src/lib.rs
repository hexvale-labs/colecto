//! Typed SDK for Colecto extensions written in Rust and compiled to
//! `wasm32-unknown-unknown`.
//!
//! You write plain functions and register them with [`export!`]; the SDK reads the
//! action, calls the matching function and emits its result. Service calls, settings
//! and the extension bus go through [`Context`]. Bus messages are types that implement
//! [`BusRequest`] or [`BusEvent`], declared with [`bus_request!`] and [`bus_event!`];
//! the bundled extensions' surfaces are ready-made in [`bundled`].
//!
//! ```ignore
//! use colecto_extension::prelude::*;
//!
//! fn panel(event: PanelEvent, cx: &mut Context) -> Result<View> {
//!     let count = cx.messages()?.len();
//!     Ok(View::new("Inbox", format!("{count} messages")).action("Refresh"))
//! }
//!
//! colecto_extension::export! { panel: panel }
//! ```
//!
//! This crate forbids `unsafe` code; the raw imports live in `colecto-extension-sys`.
//! Everything the SDK reads from the Host ignores unknown fields, so a newer Host can
//! add fields without breaking installed extensions.

pub mod bundled;
mod bus;
mod context;
mod dispatch;
#[cfg(not(target_arch = "wasm32"))]
pub mod testing;
mod types;

pub use bus::{BusEvent, BusRequest};
pub use context::Context;
pub use types::*;
pub use {serde, serde_json};

/// Everything a typical extension needs, plus serde's derive traits.
pub mod prelude {
    pub use crate::{
        BusEvent, BusRequest, Context, Error, Frame, Overlay, PanelEvent, Rect, Result, Sprite,
        View,
    };
    pub use serde::{Deserialize, Serialize};
}

/// Why an SDK call failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Error {
    /// The Host refused the bridge call: the per-invocation call limit, a size limit, or
    /// the code is not running inside Colecto.
    Bridge,
    /// The Host ran the operation and reported this error, for example
    /// `Capability not granted`.
    Service(String),
    /// A value could not be converted to or from JSON.
    Json(String),
    /// An error raised by extension code with [`Error::other`].
    Other(String),
}

impl Error {
    /// An error of your own, for example to decline a bus request.
    pub fn other(message: impl Into<String>) -> Self {
        Self::Other(message.into())
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bridge => f.write_str("the Host refused the call"),
            Self::Service(message) | Self::Other(message) => f.write_str(message),
            Self::Json(message) => write!(f, "invalid JSON: {message}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error.to_string())
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Defines the module's `run` export and routes each invocation to your functions.
///
/// Entries may come in any order, each at most once; an extension has either a
/// `panel` or an `overlay`, matching its manifest `slot`:
///
/// ```ignore
/// colecto_extension::export! {
///     panel: my_panel,             // fn(PanelEvent, &mut Context) -> Result<View>
///     overlay: my_overlay,         // fn(Frame<S>, &mut Context) -> Result<Overlay<S>>
///     requests: [word_count],      // fn(R: BusRequest, &mut Context) -> Result<R::Response>
///     events: [on_message_opened], // fn(E: BusEvent, &mut Context) -> Result<()>
/// }
/// ```
///
/// Message types are taken from each function's first parameter. Bus events only
/// arrive if the manifest lists them under `[bus] subscribe`.
///
/// It also defines `fn colecto_main()`, which runs one invocation; tests drive it
/// through [`testing::TestHost::run`].
#[macro_export]
macro_rules! export {
    ($($entries:tt)*) => {
        fn colecto_main() {
            let mut dispatch = $crate::__private::Dispatch::begin();
            $crate::__export_entries!(dispatch $($entries)*);
            dispatch.finish();
        }
        $crate::__private::export_run!(colecto_main);
    };
}

#[doc(hidden)]
#[macro_export]
macro_rules! __export_entries {
    ($d:ident) => {};
    ($d:ident panel: $f:path $(, $($rest:tt)*)?) => {
        $d.panel($f);
        $crate::__private::handler_record!(b'p', "", "");
        $crate::__export_entries!($d $($($rest)*)?);
    };
    ($d:ident overlay: $f:path $(, $($rest:tt)*)?) => {
        $d.overlay($f);
        $crate::__private::handler_record!(b'o', "", "");
        $crate::__export_entries!($d $($($rest)*)?);
    };
    ($d:ident requests: [$($f:path),* $(,)?] $(, $($rest:tt)*)?) => {
        $(
            $d.request($f);
            $crate::__private::handler_record!(
                b'r',
                $crate::__private::request_names(&$f).0,
                $crate::__private::request_names(&$f).1
            );
        )*
        $crate::__export_entries!($d $($($rest)*)?);
    };
    ($d:ident events: [$($f:path),* $(,)?] $(, $($rest:tt)*)?) => {
        $(
            $d.event($f);
            $crate::__private::handler_record!(
                b'e',
                $crate::__private::event_names(&$f).0,
                $crate::__private::event_names(&$f).1
            );
        )*
        $crate::__export_entries!($d $($($rest)*)?);
    };
}

#[doc(hidden)]
pub mod __private {
    pub use crate::bus::check_names;
    pub use crate::dispatch::Dispatch;
    use crate::{BusEvent, BusRequest, Context, Result};
    pub use colecto_extension_sys::{export_run, handler_record};

    /// `(bus, name)` of the request a handler function answers, inferred from its type.
    pub const fn request_names<R, F>(_: &F) -> (&'static str, &'static str)
    where
        R: BusRequest,
        F: FnOnce(R, &mut Context) -> Result<R::Response>,
    {
        (R::BUS, R::NAME)
    }

    /// `(bus, name)` of the event a handler function receives, inferred from its type.
    pub const fn event_names<E, F>(_: &F) -> (&'static str, &'static str)
    where
        E: BusEvent,
        F: FnOnce(E, &mut Context) -> Result<()>,
    {
        (E::BUS, E::NAME)
    }
}
