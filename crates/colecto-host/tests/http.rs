use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use colecto_core::MailStore;
use colecto_host::app as host_app;

fn app(store: MailStore, authority: String, assets: std::path::PathBuf) -> axum::Router {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(assets.join("owner-token"))
        .unwrap()
        .write_all("a".repeat(64).as_bytes())
        .unwrap();
    host_app(store, authority, assets.join("assets"))
}
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
    let router = authenticated(router).await;
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
    let router = authenticated(router).await;
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
    let router = authenticated(router).await;
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
    let router = authenticated(router).await;
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
    let router = authenticated(router).await;
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
    let router = authenticated(create()).await;
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
    let restarted = authenticated(create()).await;
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

async fn authenticated(router: axum::Router) -> axum::Router {
    let bearer = owner_bearer(&router).await;
    router.layer(axum::middleware::from_fn(
        move |mut req: axum::extract::Request, next: axum::middleware::Next| {
            req.headers_mut()
                .insert("authorization", bearer.parse().unwrap());
            async move { next.run(req).await }
        },
    ))
}

async fn owner_bearer(router: &axum::Router) -> String {
    let bearer = format!("Bearer {}", "a".repeat(64));
    let mut validate = request(
        "POST",
        "/api/v1/session",
        "host",
        "http://127.0.0.1:4173",
        "{}",
    );
    validate
        .headers_mut()
        .insert("authorization", bearer.parse().unwrap());
    let response = router.clone().oneshot(validate).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response.headers().contains_key("set-cookie"));
    bearer
}

#[tokio::test]
async fn api_keys_reject_missing_invalid_authentication_and_forged_host_headers() {
    let dir = tempfile::tempdir().unwrap();
    let router = app(
        MailStore::open(dir.path()).unwrap(),
        "127.0.0.1:4173".into(),
        dir.path().into(),
    );
    for path in ["/api/v1/api-keys", "/api/v1/accounts", "/api/v1/messages"] {
        assert_eq!(
            router
                .clone()
                .oneshot(request("GET", path, "host", "http://127.0.0.1:4173", ""))
                .await
                .unwrap()
                .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    let bearer = owner_bearer(&router).await;
    let mut mint = request(
        "POST",
        "/api/v1/api-keys",
        "host",
        "http://127.0.0.1:4173",
        r#"{"name":"Assistant","accounts":["demo"],"capabilities":["messages.list","messages.send"]}"#,
    );
    mint.headers_mut()
        .insert("authorization", bearer.parse().unwrap());
    let response = router.clone().oneshot(mint).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let minted: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    let secret = minted["secret"].as_str().unwrap();
    let id = minted["key"]["id"].as_str().unwrap();
    for (method, path, account, token, expected) in [
        ("GET", "/api/v1/messages", "demo", secret, 200),
        ("GET", "/api/v1/messages", "other", secret, 403),
        ("GET", "/api/v1/drafts", "demo", secret, 403),
        ("GET", "/api/v1/api-keys", "demo", secret, 403),
        ("GET", "/api/v1/accounts", "demo", secret, 403),
        ("POST", "/api/v1/session", "demo", secret, 403),
        ("POST", "/api/v1/send", "demo", secret, 400),
        ("GET", "/api/v1/messages", "demo", "invalid", 401),
    ] {
        let mut req = request(method, path, "host", "http://127.0.0.1:4173", "{}");
        req.headers_mut()
            .insert("authorization", format!("Bearer {token}").parse().unwrap());
        req.headers_mut()
            .insert("cookie", "colecto_owner=legacy-cookie".parse().unwrap());
        req.headers_mut()
            .insert("x-colecto-account", account.parse().unwrap());
        assert_eq!(
            router.clone().oneshot(req).await.unwrap().status().as_u16(),
            expected,
            "{method} {path} {account}"
        );
    }
    let mut revoke = request(
        "DELETE",
        &format!("/api/v1/api-keys/{id}"),
        "host",
        "http://127.0.0.1:4173",
        "{}",
    );
    revoke
        .headers_mut()
        .insert("authorization", bearer.parse().unwrap());
    assert_eq!(
        router.clone().oneshot(revoke).await.unwrap().status(),
        StatusCode::OK
    );
    let mut req = request(
        "GET",
        "/api/v1/messages",
        "inbox",
        "http://127.0.0.1:4173",
        "",
    );
    req.headers_mut()
        .insert("authorization", format!("Bearer {secret}").parse().unwrap());
    assert_eq!(
        router.oneshot(req).await.unwrap().status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn owner_bootstrap_rejects_forged_host_missing_origin_and_invalid_bearer() {
    let dir = tempfile::tempdir().unwrap();
    let router = app(
        MailStore::open(dir.path()).unwrap(),
        "127.0.0.1:4173".into(),
        dir.path().into(),
    );
    for mode in ["host", "origin", "missing-origin", "bearer"] {
        let mut req = request(
            "POST",
            "/api/v1/session",
            "host",
            "http://127.0.0.1:4173",
            "{}",
        );
        let expected = if mode == "bearer" {
            StatusCode::UNAUTHORIZED
        } else {
            StatusCode::FORBIDDEN
        };
        match mode {
            "host" => {
                req.headers_mut()
                    .insert("host", "evil.test".parse().unwrap());
            }
            "origin" => {
                req.headers_mut()
                    .insert("origin", "https://evil.test".parse().unwrap());
            }
            "missing-origin" => {
                req.headers_mut().remove("origin");
            }
            _ => {
                req.headers_mut()
                    .insert("authorization", "Bearer invalid".parse().unwrap());
            }
        }
        assert_eq!(
            router.clone().oneshot(req).await.unwrap().status(),
            expected,
            "{mode}"
        );
    }
}

#[test]
fn key_secrets_are_not_persisted_and_expiry_revocation_survive_restart() {
    use colecto_extension_api::Capability;
    use colecto_host::api_keys::{ApiKeys, KeyInput};
    let dir = tempfile::tempdir().unwrap();
    let mut keys = ApiKeys::open(dir.path()).unwrap();
    let (metadata, secret) = keys
        .mint(
            KeyInput {
                name: "Assistant".into(),
                accounts: vec!["demo".into()],
                capabilities: vec![Capability::MessagesList],
                expires_at: Some(200),
            },
            100,
        )
        .unwrap();
    let persisted = std::fs::read_to_string(dir.path().join("api-keys.json")).unwrap();
    assert!(!persisted.contains(&secret));
    assert!(!persisted.contains(secret.split_once('.').unwrap().1));
    let public = serde_json::to_string(&keys.list()).unwrap();
    assert!(!public.contains("verifier"));
    assert!(!public.contains("secret"));
    drop(keys);
    let mut keys = ApiKeys::open(dir.path()).unwrap();
    assert!(keys.authenticate(&secret, 199).is_ok());
    assert!(keys.allows(&metadata.id, "demo", Capability::MessagesList, 199));
    assert!(!keys.allows(&metadata.id, "new-account", Capability::MessagesList, 199));
    assert!(keys.authenticate(&secret, 200).is_err());
    keys.revoke(&metadata.id, 199).unwrap();
    drop(keys);
    let mut keys = ApiKeys::open(dir.path()).unwrap();
    assert!(keys.authenticate(&secret, 199).is_err());
    assert_eq!(keys.list()[0].last_used_at, Some(199));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(dir.path().join("api-keys.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o077,
            0
        );
    }
    std::fs::write(
        dir.path().join("api-keys.json"),
        "{\"version\":99,\"keys\":[]}",
    )
    .unwrap();
    assert!(ApiKeys::open(dir.path()).is_err());
}

#[test]
fn missing_and_corrupt_policy_reject_existing_key_and_recent_use_does_not_rewrite_policy() {
    use colecto_extension_api::Capability;
    use colecto_host::api_keys::{ApiKeys, KeyInput};
    let dir = tempfile::tempdir().unwrap();
    let mut keys = ApiKeys::open(dir.path()).unwrap();
    let (_, secret) = keys
        .mint(
            KeyInput {
                name: "Assistant".into(),
                accounts: vec!["demo".into()],
                capabilities: vec![Capability::MessagesList],
                expires_at: None,
            },
            100,
        )
        .unwrap();
    keys.authenticate(&secret, 100).unwrap();
    let path = dir.path().join("api-keys.json");
    let policy = std::fs::read(&path).unwrap();
    keys.authenticate(&secret, 101).unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), policy);
    assert_eq!(keys.list()[0].last_used_at, Some(100));
    std::fs::write(&path, "corrupt").unwrap();
    assert!(keys.authenticate(&secret, 102).is_err());
    std::fs::write(&path, &policy).unwrap();
    assert!(keys.authenticate(&secret, 102).is_ok());
    std::fs::remove_file(path).unwrap();
    assert!(keys.authenticate(&secret, 103).is_err());
}

#[test]
fn stale_policy_instances_cannot_resurrect_a_revoked_key() {
    use colecto_extension_api::Capability;
    use colecto_host::api_keys::{ApiKeys, KeyInput};
    let dir = tempfile::tempdir().unwrap();
    let input = || KeyInput {
        name: "Assistant".into(),
        accounts: vec!["demo".into()],
        capabilities: vec![Capability::MessagesList],
        expires_at: None,
    };
    let mut first = ApiKeys::open(dir.path()).unwrap();
    let (key, secret) = first.mint(input(), 100).unwrap();
    let mut second = ApiKeys::open(dir.path()).unwrap();
    second.revoke(&key.id, 101).unwrap();
    first.mint(input(), 102).unwrap();
    assert!(first.authenticate(&secret, 103).is_err());
    assert!(
        ApiKeys::open(dir.path())
            .unwrap()
            .authenticate(&secret, 103)
            .is_err()
    );
}

#[tokio::test]
async fn local_owner_bootstrap_requires_a_private_os_user_secret_not_just_forged_headers() {
    let dir = tempfile::tempdir().unwrap();
    let router = app(
        MailStore::open(dir.path()).unwrap(),
        "127.0.0.1:4173".into(),
        dir.path().into(),
    );
    let response = router
        .oneshot(request(
            "POST",
            "/api/v1/session",
            "host",
            "http://127.0.0.1:4173",
            "{}",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn cookie_only_owner_requests_are_rejected_even_after_valid_bearer_validation() {
    let dir = tempfile::tempdir().unwrap();
    let router = app(
        MailStore::open(dir.path()).unwrap(),
        "127.0.0.1:4173".into(),
        dir.path().into(),
    );
    let mut validate = request(
        "POST",
        "/api/v1/session",
        "host",
        "http://127.0.0.1:4173",
        "{}",
    );
    validate.headers_mut().insert(
        "authorization",
        format!("Bearer {}", "a".repeat(64)).parse().unwrap(),
    );
    let response = router.clone().oneshot(validate).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response
        .headers()
        .get("set-cookie")
        .map(|value| {
            value
                .to_str()
                .unwrap()
                .split(';')
                .next()
                .unwrap()
                .to_owned()
        })
        .unwrap_or_else(|| "colecto_owner=legacy-cookie".into());
    for path in ["/api/v1/accounts", "/api/v1/events", "/api/v1/messages"] {
        let mut req = request("GET", path, "host", "http://127.0.0.1:4173", "");
        req.headers_mut().insert("cookie", cookie.parse().unwrap());
        assert_eq!(
            router.clone().oneshot(req).await.unwrap().status(),
            StatusCode::UNAUTHORIZED,
            "{path}"
        );
    }
    assert!(!response.headers().contains_key("set-cookie"));
}
