//! In-process C-ABI FFI bridge over `colecto-service::MailService` and
//! `colecto-daemon::Daemon` for Flutter desktop & mobile standalone builds.
//!
//! Dispatches requests directly to the in-process Axum router via
//! `tower::ServiceExt::oneshot` with **zero loopback TCP ports** open, and
//! exposes live `ServiceEvent` polling for Dart's `LocalFfiBackend`.

use axum::{
    Router,
    body::Body,
    http::{HeaderValue, Method, Request, header},
};
use base64::Engine as _;
use colecto_daemon::{AccessPolicy, Assets, Daemon};
use colecto_service::{MailService, WindowGuard, events::ServiceEvent, secrets::open_accounts};
use serde::{Deserialize, Serialize};
use std::{
    ffi::{CStr, CString, c_char},
    path::PathBuf,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};
use tokio::{
    runtime::Runtime,
    sync::{broadcast, watch},
};
use tower::ServiceExt;

const FFI_AUTHORITY: &str = "colecto.local";
const FFI_ORIGIN: &str = "http://colecto.local";
const MAX_FFI_BODY_BYTES: usize = 64 * 1024 * 1024;
const DEFAULT_SYNC_SECONDS: u64 = 60;

struct FfiSession {
    runtime: Runtime,
    service: MailService,
    router: Router,
    token: String,
    events: Mutex<broadcast::Receiver<ServiceEvent>>,
    stop_tx: watch::Sender<bool>,
    _window: WindowGuard,
}

impl FfiSession {
    fn shutdown(&self) {
        let _ = self.stop_tx.send(true);
        self.service.close();
    }
}

static SESSION: OnceLock<Mutex<Option<Arc<FfiSession>>>> = OnceLock::new();

fn session_slot() -> &'static Mutex<Option<Arc<FfiSession>>> {
    SESSION.get_or_init(|| Mutex::new(None))
}

fn current_session() -> Option<Arc<FfiSession>> {
    session_slot()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

#[derive(Deserialize)]
struct FfiRequest {
    method: String,
    path: String,
    #[serde(default)]
    account: Option<String>,
    #[serde(default)]
    body: Option<String>,
}

#[derive(Serialize)]
struct FfiResponse {
    status: u16,
    content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_disposition: Option<String>,
    body: String,
    is_base64: bool,
}

#[derive(Serialize)]
struct FfiEventEnvelope {
    event: String,
    data: serde_json::Value,
}

/// Safe Rust entry point used by both C-ABI exports and unit tests.
pub fn init_in_process(data_dir: Option<&str>) -> Result<(), String> {
    // Close any previous session before opening the database again.
    shutdown_in_process();

    let dir = match data_dir.map(str::trim).filter(|s| !s.is_empty()) {
        Some(path) => PathBuf::from(path),
        None => default_data_dir(),
    };

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .map_err(|e| format!("Failed to start Tokio runtime: {e}"))?;

    let accounts = open_accounts(&dir).map_err(|e| e.to_string())?;
    let service = MailService::new(accounts)?;
    let window = service.open_window();
    let policy = AccessPolicy::local_ephemeral(FFI_AUTHORITY.to_owned())?;
    let token = policy.token().to_owned();
    let daemon = Daemon::new(service.clone(), policy, Assets::None)?;
    let router = daemon.router();
    let events = Mutex::new(service.subscribe());

    let (stop_tx, stop_rx) = watch::channel(false);
    let bg_service = service.clone();
    runtime.spawn(async move {
        bg_service
            .run_background(Duration::from_secs(DEFAULT_SYNC_SECONDS), stop_rx)
            .await;
    });

    let mut guard = session_slot()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *guard = Some(Arc::new(FfiSession {
        runtime,
        service,
        router,
        token,
        events,
        stop_tx,
        _window: window,
    }));
    Ok(())
}

/// Safe Rust in-process request dispatcher (zero TCP ports).
pub fn invoke_json(request_json: &str) -> String {
    let parsed: FfiRequest = match serde_json::from_str(request_json) {
        Ok(req) => req,
        Err(e) => {
            return error_response(400, &format!("Invalid FFI request JSON: {e}"));
        }
    };

    let Some(session) = current_session() else {
        return error_response(503, "Colecto FFI session is not initialized.");
    };

    let method = match Method::from_bytes(parsed.method.trim().as_bytes()) {
        Ok(m) => m,
        Err(_) => return error_response(400, "Invalid HTTP method."),
    };
    let path = if parsed.path.starts_with('/') {
        parsed.path
    } else {
        format!("/{}", parsed.path)
    };

    let mut builder = Request::builder()
        .method(method)
        .uri(&path)
        .header(header::HOST, FFI_AUTHORITY)
        .header(header::ORIGIN, FFI_ORIGIN)
        .header(header::AUTHORIZATION, format!("Bearer {}", session.token));

    if let Some(account) = parsed.account.as_deref().filter(|s| !s.is_empty())
        && let Ok(val) = HeaderValue::from_str(account)
    {
        builder = builder.header("x-colecto-account", val);
    }

    let body_bytes = match parsed.body {
        Some(text) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            text.into_bytes()
        }
        None => Vec::new(),
    };

    let request = match builder.body(Body::from(body_bytes)) {
        Ok(r) => r,
        Err(e) => return error_response(400, &format!("Failed to build request: {e}")),
    };

    let router = session.router.clone();
    let response = session.runtime.block_on(async move {
        let Ok(resp) = router.oneshot(request).await;
        let status = resp.status().as_u16();
        let content_type = resp
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("application/json")
            .to_owned();
        let content_disposition = resp
            .headers()
            .get(header::CONTENT_DISPOSITION)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let bytes = match axum::body::to_bytes(resp.into_body(), MAX_FFI_BODY_BYTES).await {
            Ok(b) => b,
            Err(_) => {
                return FfiResponse {
                    status: 413,
                    content_type: "application/json".to_owned(),
                    content_disposition: None,
                    body: serde_json::json!({
                        "error": "Response exceeds the 64 MiB in-process FFI limit.",
                        "category": "storage_or_service"
                    })
                    .to_string(),
                    is_base64: false,
                };
            }
        };
        let is_binary =
            !content_type.starts_with("application/json") && !content_type.starts_with("text/");
        let body = if is_binary {
            base64::engine::general_purpose::STANDARD.encode(&bytes)
        } else {
            String::from_utf8_lossy(&bytes).into_owned()
        };
        FfiResponse {
            status,
            content_type,
            content_disposition,
            body,
            is_base64: is_binary,
        }
    });

    serde_json::to_string(&response).unwrap_or_else(|_| error_response(500, "Serialization error"))
}

/// Polls the next live `ServiceEvent`, waiting up to `timeout_ms` milliseconds.
pub fn poll_event_json(timeout_ms: u64) -> Option<String> {
    let session = current_session()?;
    let mut rx = session
        .events
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    let event = session.runtime.block_on(async {
        tokio::time::timeout(Duration::from_millis(timeout_ms.min(5000)), rx.recv()).await
    });

    match event {
        Ok(Ok(ev)) => {
            let name = ev.event_name();
            if name.is_empty() {
                return None;
            }
            let payload: serde_json::Value =
                serde_json::from_str(&ev.payload_json(Some(session.service.open_windows())))
                    .unwrap_or_default();
            serde_json::to_string(&FfiEventEnvelope {
                event: name.to_owned(),
                data: payload,
            })
            .ok()
        }
        Ok(Err(broadcast::error::RecvError::Lagged(_))) => {
            serde_json::to_string(&FfiEventEnvelope {
                event: "resync".to_owned(),
                data: serde_json::json!({}),
            })
            .ok()
        }
        _ => None,
    }
}

/// Shuts down the active in-process `MailService` session.
pub fn shutdown_in_process() {
    let prev = {
        let mut guard = session_slot()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        guard.take()
    };
    if let Some(session) = prev {
        session.shutdown();
    }
}

fn error_response(status: u16, message: &str) -> String {
    let resp = FfiResponse {
        status,
        content_type: "application/json".to_owned(),
        content_disposition: None,
        body: serde_json::json!({ "error": message, "category": "storage_or_service" }).to_string(),
        is_base64: false,
    };
    serde_json::to_string(&resp).unwrap_or_default()
}

/// Bundle identifier the app data directory is named after, shared with v0.9.0 and
/// earlier so their `colecto.db` is found and migrated forward.
const APP_IDENTIFIER: &str = "com.kogical.colecto";

/// The desktop app data directory: `COLECTO_DATA_DIR`, else the platform's data
/// directory joined with [`APP_IDENTIFIER`] (the same place Tauri's `app_data_dir`
/// used). Android and iOS pass their sandbox directory from Dart instead.
fn default_data_dir() -> PathBuf {
    data_dir_from(|name| std::env::var_os(name))
}

fn data_dir_from(env: impl Fn(&str) -> Option<std::ffi::OsString>) -> PathBuf {
    if let Some(explicit) = env("COLECTO_DATA_DIR").filter(|value| !value.is_empty()) {
        return PathBuf::from(explicit);
    }
    let var = |name: &str| {
        env(name)
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
    };
    let base = if cfg!(windows) {
        var("APPDATA")
    } else if cfg!(target_os = "macos") {
        var("HOME").map(|home| home.join("Library/Application Support"))
    } else {
        var("XDG_DATA_HOME").or_else(|| var("HOME").map(|home| home.join(".local/share")))
    };
    base.map_or_else(
        || PathBuf::from(".colecto"),
        |base| base.join(APP_IDENTIFIER),
    )
}

fn into_c_string(text: String) -> *mut c_char {
    CString::new(text)
        .unwrap_or_else(|_| CString::new("{}").expect("valid fallback"))
        .into_raw()
}

/// Initialize the in-process `MailService` with an optional UTF-8 `data_dir` path.
/// Returns a newly allocated JSON string (`{"ok":true}` or `{"error":"..."}`).
/// Caller must free the returned pointer with `colecto_ffi_free_string`.
///
/// # Safety
/// `data_dir` must be either null or a valid, NUL-terminated UTF-8 C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn colecto_ffi_init(data_dir: *const c_char) -> *mut c_char {
    let dir = if data_dir.is_null() {
        None
    } else {
        // SAFETY: Caller passes a valid NUL-terminated UTF-8 string or null.
        unsafe { CStr::from_ptr(data_dir) }.to_str().ok()
    };
    let result = match init_in_process(dir) {
        Ok(()) => serde_json::json!({ "ok": true }).to_string(),
        Err(error) => serde_json::json!({ "ok": false, "error": error }).to_string(),
    };
    into_c_string(result)
}

/// Invoke an in-process `/api/v1/*` request encoded as JSON.
/// Caller must free the returned pointer with `colecto_ffi_free_string`.
///
/// # Safety
/// `request_json` must be either null or a valid, NUL-terminated UTF-8 C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn colecto_ffi_request(request_json: *const c_char) -> *mut c_char {
    if request_json.is_null() {
        return into_c_string(error_response(400, "Null request pointer"));
    }
    // SAFETY: Caller passes a valid NUL-terminated UTF-8 string.
    let Ok(text) = unsafe { CStr::from_ptr(request_json) }.to_str() else {
        return into_c_string(error_response(400, "Request is not valid UTF-8"));
    };
    into_c_string(invoke_json(text))
}

/// Poll the next live `ServiceEvent` as JSON, or return NULL on timeout.
/// Caller must free non-null pointers with `colecto_ffi_free_string`.
#[unsafe(no_mangle)]
pub extern "C" fn colecto_ffi_poll_event(timeout_ms: u64) -> *mut c_char {
    match poll_event_json(timeout_ms) {
        Some(json) => into_c_string(json),
        None => std::ptr::null_mut(),
    }
}

/// Shutdown the in-process `MailService`.
#[unsafe(no_mangle)]
pub extern "C" fn colecto_ffi_shutdown() {
    shutdown_in_process();
}

/// Free a string previously returned by `colecto_ffi_*`.
///
/// # Safety
/// `ptr` must be either null or a pointer previously returned by this library
/// that has not yet been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn colecto_ffi_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    // SAFETY: Pointer was allocated by `CString::into_raw` in this library.
    drop(unsafe { CString::from_raw(ptr) });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_process_ffi_serves_version_and_preferences_without_tcp_port() {
        let tmp = tempfile::tempdir().expect("tempdir");
        init_in_process(tmp.path().to_str()).expect("init ffi");

        let ver_raw = invoke_json(r#"{"method":"GET","path":"/api/v1/version"}"#);
        let ver_resp: serde_json::Value = serde_json::from_str(&ver_raw).expect("parse response");
        assert_eq!(ver_resp["status"], 200);
        let ver_body: serde_json::Value =
            serde_json::from_str(ver_resp["body"].as_str().unwrap()).expect("parse version body");
        assert_eq!(ver_body["api"], "v1");

        let pref_raw = invoke_json(
            r#"{"method":"POST","path":"/api/v1/preferences","body":"{\"list_density\":\"compact\"}"}"#,
        );
        let pref_resp: serde_json::Value = serde_json::from_str(&pref_raw).expect("parse prefs");
        assert_eq!(pref_resp["status"], 200);
        let pref_body: serde_json::Value =
            serde_json::from_str(pref_resp["body"].as_str().unwrap()).expect("parse prefs body");
        assert_eq!(pref_body["list_density"], "compact");

        shutdown_in_process();
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn default_data_dir_matches_the_tauri_app() {
        let env = |pairs: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                pairs
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| value.into())
            }
        };
        assert_eq!(
            data_dir_from(env(&[("HOME", "/home/u")])),
            PathBuf::from("/home/u/.local/share/com.kogical.colecto")
        );
        assert_eq!(
            data_dir_from(env(&[("HOME", "/home/u"), ("XDG_DATA_HOME", "/data")])),
            PathBuf::from("/data/com.kogical.colecto")
        );
        assert_eq!(
            data_dir_from(env(&[("HOME", "/home/u"), ("XDG_DATA_HOME", "relative")])),
            PathBuf::from("/home/u/.local/share/com.kogical.colecto")
        );
        assert_eq!(
            data_dir_from(env(&[("HOME", "/home/u"), ("COLECTO_DATA_DIR", "/x")])),
            PathBuf::from("/x")
        );
    }
}
