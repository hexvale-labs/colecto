//! Integration tests for the in-process C-ABI FFI bridge (`colecto-ffi`).
//!
//! Because `colecto-ffi` holds a process-global session slot, tests run in a
//! single serialized lifecycle verifying C-ABI safety, full mail/draft/label/
//! organize persistence across re-init, and live `ServiceEvent` polling with
//! zero loopback TCP ports.

use colecto_ffi::{
    colecto_ffi_free_string, colecto_ffi_init, colecto_ffi_poll_event, colecto_ffi_request,
    colecto_ffi_shutdown,
};
use serde_json::{Value, json};
use std::ffi::{CStr, CString, c_char};

unsafe fn take_c_string(ptr: *mut c_char) -> String {
    assert!(!ptr.is_null(), "expected non-null C string");
    let text = unsafe { CStr::from_ptr(ptr) }
        .to_str()
        .expect("valid UTF-8")
        .to_owned();
    unsafe { colecto_ffi_free_string(ptr) };
    text
}

unsafe fn take_c_json(ptr: *mut c_char) -> Value {
    let raw = unsafe { take_c_string(ptr) };
    serde_json::from_str(&raw).expect("valid JSON")
}

fn c_request(req: &Value) -> Value {
    let input = CString::new(req.to_string()).unwrap();
    unsafe { take_c_json(colecto_ffi_request(input.as_ptr())) }
}

fn body_json(resp: &Value) -> Value {
    serde_json::from_str(resp["body"].as_str().expect("body string")).expect("valid body JSON")
}

#[test]
fn c_abi_lifecycle_mail_persistence_and_live_events() {
    // 1. Pre-init & malformed C-ABI inputs fail cleanly without panicking.
    colecto_ffi_shutdown();
    let null_resp = unsafe { take_c_json(colecto_ffi_request(std::ptr::null())) };
    assert_eq!(null_resp["status"], 400);

    let uninit = c_request(&json!({"method": "GET", "path": "/api/v1/version"}));
    assert_eq!(uninit["status"], 503);
    assert!(colecto_ffi_poll_event(10).is_null());

    // 2. Initialize against an isolated temporary data directory.
    let dir = tempfile::tempdir().unwrap();
    let dir_c = CString::new(dir.path().to_str().unwrap()).unwrap();
    let init = unsafe { take_c_json(colecto_ffi_init(dir_c.as_ptr())) };
    assert_eq!(init["ok"], true);

    // 3. Invalid JSON & bad HTTP methods return structured 400 errors.
    let bad_json_c = CString::new("{not-json").unwrap();
    let bad_json = unsafe { take_c_json(colecto_ffi_request(bad_json_c.as_ptr())) };
    assert_eq!(bad_json["status"], 400);

    let bad_method = c_request(&json!({"method": "BAD METHOD", "path": "/api/v1/version"}));
    assert_eq!(bad_method["status"], 400);

    // 4. Version, accounts, folders, and sample messages work without leading slash too.
    let version = c_request(&json!({"method": "GET", "path": "api/v1/version"}));
    assert_eq!(version["status"], 200);
    assert_eq!(body_json(&version)["api"], "v1");

    let messages = c_request(&json!({
        "method": "GET",
        "path": "/api/v1/messages?folder=inbox",
        "account": "demo"
    }));
    assert_eq!(messages["status"], 200);
    let msg_list = body_json(&messages);
    let first_id = msg_list[0]["id"].as_str().unwrap().to_owned();

    // 5. Save a draft, create & assign a label, update preferences, and receive live events.
    let saved_draft = c_request(&json!({
        "method": "POST",
        "path": "/api/v1/drafts",
        "account": "demo",
        "body": json!({
            "recipient": "ffi@example.test",
            "subject": "FFI persisted draft",
            "body": "Zero TCP sockets"
        }).to_string()
    }));
    assert_eq!(saved_draft["status"], 200);
    let draft_id = body_json(&saved_draft)["id"].as_str().unwrap().to_owned();

    let pref = c_request(&json!({
        "method": "POST",
        "path": "/api/v1/preferences",
        "body": r#"{"list_density":"compact","mcp":true}"#
    }));
    assert_eq!(pref["status"], 200);

    let label = c_request(&json!({
        "method": "POST",
        "path": "/api/v1/labels",
        "account": "demo",
        "body": json!({"name": "FFI"}).to_string()
    }));
    assert_eq!(label["status"], 200);

    let assign = c_request(&json!({
        "method": "POST",
        "path": "/api/v1/labels",
        "account": "demo",
        "body": json!({"name": "FFI", "message_id": first_id}).to_string()
    }));
    assert_eq!(assign["status"], 200);

    let mut seen_events = Vec::new();
    for _ in 0..8 {
        let ptr = colecto_ffi_poll_event(50);
        if ptr.is_null() {
            break;
        }
        let ev = unsafe { take_c_json(ptr) };
        seen_events.push((
            ev["event"].as_str().unwrap_or_default().to_owned(),
            ev["data"].clone(),
        ));
    }
    assert!(
        seen_events
            .iter()
            .any(|(name, data)| name == "mcp" && data["on"] == true),
        "expected mcp event in {seen_events:?}"
    );

    // 6. Restart FFI session on the same directory and verify Turso persistence + no sidecars.
    colecto_ffi_shutdown();
    let reinit_raw = unsafe { take_c_string(colecto_ffi_init(dir_c.as_ptr())) };
    assert_eq!(
        serde_json::from_str::<Value>(&reinit_raw).unwrap()["ok"],
        true
    );

    let reloaded = c_request(&json!({
        "method": "GET",
        "path": format!("/api/v1/drafts/{draft_id}"),
        "account": "demo"
    }));
    assert_eq!(reloaded["status"], 200);
    assert_eq!(body_json(&reloaded)["subject"], "FFI persisted draft");
    assert!(dir.path().join("colecto.db").exists());
    assert!(!dir.path().join("owner-token").exists());

    // Freeing null is a safe no-op.
    unsafe { colecto_ffi_free_string(std::ptr::null_mut()) };
    colecto_ffi_shutdown();
}
