use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use colecto_core::MailStore;
use colecto_host::app;
use tower::ServiceExt;

fn request(method: &str, path: &str, extension: &str, origin: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:4173")
        .header("origin", origin)
        .header("x-colecto-account", "demo")
        .header("x-colecto-extension", extension)
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

#[tokio::test]
async fn granted_save_and_read_share_core_state() {
    let dir = tempfile::tempdir().unwrap();
    let router = app(
        MailStore::open(dir.path()).unwrap(),
        "127.0.0.1:4173".into(),
        dir.path().into(),
    );
    let response = router
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/drafts",
            "composer",
            "http://127.0.0.1:4173",
            r#"{"recipient":"a@example.test","subject":"Saved","body":"Hello"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let draft: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 200_000).await.unwrap()).unwrap();
    let id = draft["id"].as_str().unwrap();
    let response = router
        .oneshot(request(
            "GET",
            &format!("/api/v1/drafts/{id}"),
            "composer",
            "http://127.0.0.1:4173",
            "",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let draft: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 200_000).await.unwrap()).unwrap();
    assert_eq!(draft["subject"], "Saved");
}

#[tokio::test]
async fn host_rejects_cross_origin_ungranted_and_malformed_requests() {
    let dir = tempfile::tempdir().unwrap();
    let router = app(
        MailStore::open(dir.path()).unwrap(),
        "127.0.0.1:4173".into(),
        dir.path().into(),
    );
    for (method, path, extension, origin, body, expected) in [
        (
            "POST",
            "/api/v1/drafts",
            "composer",
            "https://evil.test",
            "{}",
            403,
        ),
        ("POST", "/api/v1/drafts", "composer", "", "{}", 403),
        (
            "POST",
            "/api/v1/drafts",
            "inbox",
            "http://127.0.0.1:4173",
            "{}",
            403,
        ),
        (
            "GET",
            "/api/v1/messages",
            "unknown",
            "http://127.0.0.1:4173",
            "",
            403,
        ),
        (
            "GET",
            "/api/v1/messages/missing",
            "reader",
            "http://127.0.0.1:4173",
            "",
            404,
        ),
        (
            "POST",
            "/api/v1/drafts",
            "composer",
            "http://127.0.0.1:4173",
            "{}",
            400,
        ),
        (
            "POST",
            "/api/v1/drafts",
            "composer",
            "http://127.0.0.1:4173",
            r#"{"id":"missing","recipient":"","subject":"","body":""}"#,
            404,
        ),
    ] {
        let response = router
            .clone()
            .oneshot(request(method, path, extension, origin, body))
            .await
            .unwrap();
        assert_eq!(
            response.status().as_u16(),
            expected,
            "{method} {path} {extension} {origin}"
        );
        let error: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 200_000).await.unwrap())
                .unwrap();
        assert!(error["error"].is_string());
    }
    let mut req = request(
        "GET",
        "/api/v1/messages",
        "inbox",
        "http://127.0.0.1:4173",
        "",
    );
    req.headers_mut()
        .insert("host", "evil.test".parse().unwrap());
    assert_eq!(
        router.clone().oneshot(req).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let mut req = request(
        "GET",
        "/api/v1/messages",
        "inbox",
        "http://127.0.0.1:4173",
        "",
    );
    req.headers_mut()
        .insert("x-colecto-account", "other".parse().unwrap());
    assert_eq!(
        router.oneshot(req).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn send_is_scoped_and_demo_never_reports_delivery() {
    let dir = tempfile::tempdir().unwrap();
    let router = app(
        MailStore::open(dir.path()).unwrap(),
        "127.0.0.1:4173".into(),
        dir.path().into(),
    );
    for (extension, origin, expected) in [
        ("inbox", "http://127.0.0.1:4173", StatusCode::FORBIDDEN),
        ("composer", "https://evil.test", StatusCode::FORBIDDEN),
        ("composer", "http://127.0.0.1:4173", StatusCode::BAD_REQUEST),
    ] {
        let response = router
            .clone()
            .oneshot(request(
                "POST",
                "/api/v1/send",
                extension,
                origin,
                r#"{"recipient":"friend@example.test","subject":"Hello","body":"Keep this"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
    let response = router
        .oneshot(request(
            "GET",
            "/api/v1/account",
            "inbox",
            "http://127.0.0.1:4173",
            "",
        ))
        .await
        .unwrap();
    assert_eq!(
        to_bytes(response.into_body(), 100).await.unwrap().as_ref(),
        b"false"
    );
}

#[tokio::test]
async fn management_is_host_only_and_password_mode_needs_no_keychain() {
    let dir = tempfile::tempdir().unwrap();
    let router = app(
        MailStore::open(dir.path()).unwrap(),
        "127.0.0.1:4173".into(),
        dir.path().into(),
    );
    let denied = router
        .clone()
        .oneshot(request(
            "GET",
            "/api/v1/accounts",
            "inbox",
            "http://127.0.0.1:4173",
            "",
        ))
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let response = router
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/security",
            "host",
            "http://127.0.0.1:4173",
            r#"{"action":"protect","password":"a strong master password"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = router.clone().oneshot(request("POST", "/api/v1/accounts", "host", "http://127.0.0.1:4173", r#"{"name":"Work","address":"work@example.test","protocol":"imap","imap_host":"imap.example.test","smtp_host":"smtp.example.test","username":"work","secret":"never-return-this"}"#)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 200000).await.unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("never-return-this"));
    let info: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let account = info["id"].as_str().unwrap();
    let mut req = request(
        "POST",
        "/api/v1/drafts",
        "composer",
        "http://127.0.0.1:4173",
        r#"{"subject":"Private draft","recipient":"","body":""}"#,
    );
    req.headers_mut()
        .insert("x-colecto-account", account.parse().unwrap());
    assert_eq!(
        router.clone().oneshot(req).await.unwrap().status(),
        StatusCode::OK
    );
    let response = router
        .oneshot(request(
            "GET",
            "/api/v1/drafts",
            "inbox",
            "http://127.0.0.1:4173",
            "",
        ))
        .await
        .unwrap();
    assert_eq!(
        to_bytes(response.into_body(), 200000)
            .await
            .unwrap()
            .as_ref(),
        b"[]"
    );
}

#[tokio::test]
async fn extension_controls_are_host_only_and_revocation_reaches_service_boundary() {
    let dir = tempfile::tempdir().unwrap();
    let router = app(
        MailStore::open(dir.path()).unwrap(),
        "127.0.0.1:4173".into(),
        dir.path().into(),
    );
    let origin = "http://127.0.0.1:4173";
    for (method, path, identity, body, status) in [
        ("GET", "/api/v1/extensions", "host", "", 200),
        ("GET", "/api/v1/extensions", "reader", "", 403),
        (
            "POST",
            "/api/v1/extensions/reader",
            "reader",
            r#"{"enabled":true,"grants":["messages.read"]}"#,
            403,
        ),
        (
            "POST",
            "/api/v1/extensions/reader",
            "host",
            r#"{"enabled":true,"grants":["messages.send"]}"#,
            403,
        ),
        (
            "POST",
            "/api/v1/extensions/reader",
            "host",
            r#"{"enabled":true,"grants":["accounts.manage"]}"#,
            400,
        ),
        (
            "POST",
            "/api/v1/extensions/reader",
            "host",
            r#"{"enabled":true,"grants":[]}"#,
            200,
        ),
        ("GET", "/api/v1/messages/1", "reader", "", 403),
        (
            "POST",
            "/api/v1/extensions/inbox",
            "host",
            r#"{"enabled":false,"grants":["messages.list"]}"#,
            200,
        ),
        ("GET", "/api/v1/messages", "inbox", "", 403),
        (
            "POST",
            "/api/v1/extensions/inbox",
            "host",
            r#"{"enabled":true,"grants":["messages.list"]}"#,
            200,
        ),
        ("GET", "/api/v1/messages", "inbox", "", 200),
        ("GET", "/api/v1/drafts", "inbox", "", 403),
        (
            "POST",
            "/api/v1/extensions/external",
            "host",
            r#"{"enabled":true,"grants":[]}"#,
            403,
        ),
    ] {
        let response = router
            .clone()
            .oneshot(request(method, path, identity, origin, body))
            .await
            .unwrap();
        assert_eq!(
            response.status().as_u16(),
            status,
            "{method} {path} {identity}"
        );
    }
}

#[tokio::test]
async fn installed_wasm_uses_consented_account_grants_and_cannot_call_http_services() {
    let dir = tempfile::tempdir().unwrap();
    let authority = "127.0.0.1:4173";
    let origin = "http://127.0.0.1:4173";
    let create = || {
        app(
            MailStore::open(dir.path()).unwrap(),
            authority.into(),
            dir.path().into(),
        )
    };
    let router = create();
    let manifest = include_str!("../../../extensions/examples/connection-panel/extension.toml");
    let module = wat::parse_str(include_str!(
        "../../../extensions/examples/connection-panel/module.wat"
    ))
    .unwrap();
    let package = serde_json::json!({"manifest": manifest, "module": module}).to_string();
    let response = router
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/extensions",
            "host",
            origin,
            &package,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let run = "/api/v1/extensions/connection-panel/run";
    let response = router
        .clone()
        .oneshot(request("POST", run, "host", origin, r#"{"action":"open"}"#))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    for (policy, text) in [
        (r#"{"enabled":true,"grants":[]}"#, "Permission required"),
        (
            r#"{"enabled":true,"grants":["account.read"]}"#,
            "Account disconnected",
        ),
        (r#"{"enabled":true,"grants":[]}"#, "Permission required"),
    ] {
        let response = router
            .clone()
            .oneshot(request(
                "POST",
                "/api/v1/extensions/connection-panel",
                "host",
                origin,
                policy,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let response = router
            .clone()
            .oneshot(request(
                "POST",
                run,
                "host",
                origin,
                r#"{"action":"Refresh"}"#,
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
        assert!(body["text"].as_str().unwrap().contains(text));
    }
    let response = router
        .clone()
        .oneshot(request(
            "GET",
            "/api/v1/account",
            "connection-panel",
            origin,
            "",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    drop(router);
    let restarted = create();
    let response = restarted
        .clone()
        .oneshot(request("POST", run, "host", origin, r#"{"action":"open"}"#))
        .await
        .unwrap();
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    assert!(
        body["text"]
            .as_str()
            .unwrap()
            .contains("Permission required")
    );
    let response = restarted
        .oneshot(request(
            "DELETE",
            "/api/v1/extensions/connection-panel",
            "host",
            origin,
            "{}",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}
