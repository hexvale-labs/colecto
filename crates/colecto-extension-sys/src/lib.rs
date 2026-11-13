//! Raw bridge to the Colecto guest ABI (API 1).
//!
//! This is the only SDK crate that contains `unsafe` code. It wraps the three
//! `colecto` imports in safe byte-level functions and provides [`export_run!`], which
//! defines the `run` export. Extensions normally use the typed `colecto-extension`
//! crate instead of this one.
//!
//! On targets other than `wasm32` the imports do not exist, so the functions talk to an
//! in-thread [`mock`] host instead. That keeps extension code testable with plain
//! `cargo test`.

/// Largest action or response the Host writes into guest memory (bus actions are the
/// biggest at 64 KiB); a smaller buffer would make the import fail.
pub const BRIDGE_CAPACITY: usize = 64 * 1024;

/// Copies the current invocation's action into guest memory, or `None` if the Host
/// refused (for example when the action does not fit).
pub fn action() -> Option<Vec<u8>> {
    let mut buffer = vec![0u8; BRIDGE_CAPACITY];
    let len = imports::action(&mut buffer);
    truncate(buffer, len)
}

/// Sends one JSON service request and returns the JSON response envelope, or `None`
/// if the Host refused the call (too many calls, invalid JSON or an oversized reply).
pub fn request(input: &[u8]) -> Option<Vec<u8>> {
    let mut buffer = vec![0u8; BRIDGE_CAPACITY];
    let len = imports::request(input, &mut buffer);
    truncate(buffer, len)
}

/// Emits the invocation's output. Returns `false` if the Host rejected it; only the
/// first emit of an invocation can succeed.
pub fn emit(output: &[u8]) -> bool {
    imports::emit(output) == 0
}

fn truncate(mut buffer: Vec<u8>, len: i32) -> Option<Vec<u8>> {
    let len = usize::try_from(len)
        .ok()
        .filter(|&len| len <= buffer.len())?;
    buffer.truncate(len);
    Some(buffer)
}

/// Defines the module's `run` export, calling `$entry` (a `fn()`) once per invocation.
///
/// Use it at most once per extension, normally through `colecto_extension::export!`.
/// On non-Wasm targets it only marks `$entry` as used, so test binaries get no stray
/// symbol and builds there do not warn about dead code.
#[macro_export]
macro_rules! export_run {
    ($entry:path) => {
        #[cfg(target_arch = "wasm32")]
        #[unsafe(export_name = "run")]
        extern "C" fn __colecto_extension_run() {
            $entry()
        }
        #[cfg(not(target_arch = "wasm32"))]
        const _: fn() = $entry;
    };
}

/// Records one registered handler in the module's `colecto.handlers.v1` custom section,
/// so `cargo colecto` can compare the code with the manifest. Each record is
/// [`record`]`(kind, bus, name)`; the linker concatenates them. Kinds: `b'p'` panel,
/// `b'o'` overlay, `b'r'` bus request, `b'e'` bus event. Nothing is emitted off Wasm.
#[macro_export]
macro_rules! handler_record {
    ($kind:literal, $bus:expr, $name:expr) => {
        #[cfg(target_arch = "wasm32")]
        const _: () = {
            const BUS: &str = $bus;
            const NAME: &str = $name;
            #[unsafe(link_section = "colecto.handlers.v1")]
            #[used]
            static RECORD: [u8; 3 + BUS.len() + NAME.len()] = $crate::record($kind, BUS, NAME);
        };
    };
}

/// Encodes one handler record: kind byte, then `bus` and `name`, each prefixed by its
/// length in one byte. `N` must be `3 + bus.len() + name.len()`.
pub const fn record<const N: usize>(kind: u8, bus: &str, name: &str) -> [u8; N] {
    let (bus, name) = (bus.as_bytes(), name.as_bytes());
    assert!(N == 3 + bus.len() + name.len() && bus.len() < 256 && name.len() < 256);
    let mut out = [0u8; N];
    out[0] = kind;
    out[1] = bus.len() as u8;
    let mut i = 0;
    while i < bus.len() {
        out[2 + i] = bus[i];
        i += 1;
    }
    out[2 + bus.len()] = name.len() as u8;
    let mut i = 0;
    while i < name.len() {
        out[3 + bus.len() + i] = name[i];
        i += 1;
    }
    out
}

#[cfg(target_arch = "wasm32")]
mod imports {
    #[link(wasm_import_module = "colecto")]
    unsafe extern "C" {
        #[link_name = "action"]
        fn colecto_action(output: *mut u8, capacity: usize) -> i32;
        #[link_name = "request"]
        fn colecto_request(input: *const u8, len: usize, output: *mut u8, capacity: usize) -> i32;
        #[link_name = "emit"]
        fn colecto_emit(input: *const u8, len: usize) -> i32;
    }

    // SAFETY (all three): each pointer and length comes from a live slice, and the Host
    // only reads `len` bytes from inputs and writes at most `capacity` bytes to outputs.
    pub fn action(output: &mut [u8]) -> i32 {
        unsafe { colecto_action(output.as_mut_ptr(), output.len()) }
    }
    pub fn request(input: &[u8], output: &mut [u8]) -> i32 {
        unsafe {
            colecto_request(
                input.as_ptr(),
                input.len(),
                output.as_mut_ptr(),
                output.len(),
            )
        }
    }
    pub fn emit(input: &[u8]) -> i32 {
        unsafe { colecto_emit(input.as_ptr(), input.len()) }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imports {
    use super::mock::HOST;

    fn copy(bytes: &[u8], output: &mut [u8]) -> i32 {
        if bytes.len() > output.len() {
            return -1;
        }
        output[..bytes.len()].copy_from_slice(bytes);
        bytes.len() as i32
    }
    pub fn action(output: &mut [u8]) -> i32 {
        HOST.with_borrow(|host| host.as_ref().map_or(-1, |host| copy(&host.action, output)))
    }
    pub fn request(input: &[u8], output: &mut [u8]) -> i32 {
        // Take the handler out while it runs, so it may itself use the mock.
        let Some(mut handler) = HOST.with_borrow_mut(|h| h.as_mut().and_then(|h| h.handler.take()))
        else {
            return -1;
        };
        let response = handler(input);
        HOST.with_borrow_mut(|h| {
            if let Some(h) = h {
                h.handler = Some(handler);
            }
        });
        response.map_or(-1, |bytes| copy(&bytes, output))
    }
    pub fn emit(input: &[u8]) -> i32 {
        HOST.with_borrow_mut(|host| match host {
            Some(host) if host.emitted.is_none() => {
                host.emitted = Some(input.to_vec());
                0
            }
            _ => -1,
        })
    }
}

/// An in-thread stand-in for the Host, used on non-Wasm targets (tests).
#[cfg(not(target_arch = "wasm32"))]
pub mod mock {
    use std::cell::RefCell;

    /// Answers one raw service request; `None` behaves like a refused call.
    pub type Handler = Box<dyn FnMut(&[u8]) -> Option<Vec<u8>>>;

    pub(crate) struct Host {
        pub action: Vec<u8>,
        pub handler: Option<Handler>,
        pub emitted: Option<Vec<u8>>,
    }

    thread_local! {
        pub(crate) static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
    }

    /// Starts a mock invocation on this thread with the given action and request handler.
    pub fn begin(action: &[u8], handler: Handler) {
        HOST.set(Some(Host {
            action: action.to_vec(),
            handler: Some(handler),
            emitted: None,
        }));
    }

    /// Ends the mock invocation and returns what it emitted, if anything.
    pub fn finish() -> Option<Vec<u8>> {
        HOST.take().and_then(|host| host.emitted)
    }

    #[cfg(test)]
    mod tests {
        #[test]
        fn mirrors_host_bridge_rules() {
            assert_eq!(crate::action(), None, "no invocation running");
            super::begin(b"open", Box::new(|input| Some([b"echo:", input].concat())));
            assert_eq!(crate::action().as_deref(), Some(&b"open"[..]));
            assert_eq!(crate::request(b"x").as_deref(), Some(&b"echo:x"[..]));
            assert!(crate::emit(b"first"));
            assert!(!crate::emit(b"second"), "only the first emit counts");
            assert_eq!(super::finish().as_deref(), Some(&b"first"[..]));
        }
    }
}
