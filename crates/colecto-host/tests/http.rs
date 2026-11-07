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
