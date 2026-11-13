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
async fn message_content_needs_read_grant_and_send_accepts_attachment_sized_bodies() {
    let dir = tempfile::tempdir().unwrap();
    let router = app(
        MailStore::open(dir.path()).unwrap(),
        "127.0.0.1:4173".into(),
        dir.path().into(),
    );
    let router = authenticated(router).await;
    let origin = "http://127.0.0.1:4173";
    for (extension, expected) in [("inbox", StatusCode::FORBIDDEN), ("reader", StatusCode::OK)] {
        let response = router
            .clone()
            .oneshot(request(
                "GET",
                "/api/v1/messages/welcome/content",
                extension,
                origin,
                "",
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
    let attachment = "A".repeat(2_000_000);
    let body = format!(
        r#"{{"recipient":"a@example.test","subject":"Hi","body":"x","attachments":[{{"name":"a.bin","content_type":"application/octet-stream","data":"{attachment}"}}]}}"#
    );
    let response = router
        .oneshot(request("POST", "/api/v1/send", "composer", origin, &body))
        .await
        .unwrap();
    // Reaches the demo store (no account) instead of the global 512 KB body limit.
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
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
            r#"{"enabled":true}"#,
            403,
        ),
        (
            "POST",
            "/api/v1/extensions/reader",
            "host",
            r#"{"enabled":true,"grants":[]}"#,
            400,
        ),
        (
            "POST",
            "/api/v1/extensions/inbox",
            "host",
            r#"{"enabled":false}"#,
            200,
        ),
        ("GET", "/api/v1/messages", "inbox", "", 403),
        (
            "POST",
            "/api/v1/extensions/inbox",
            "host",
            r#"{"enabled":true}"#,
            200,
        ),
        ("GET", "/api/v1/messages", "inbox", "", 200),
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
    let manifest =
        include_str!("../../../docs/extensions/examples/connection-panel/extension.toml");
    let module = wat::parse_str(include_str!(
        "../../../docs/extensions/examples/connection-panel/module.wat"
    ))
    .unwrap();
    let package =
        serde_json::json!({"package": colecto_host::package::write(manifest, &module)}).to_string();
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
    assert_eq!(response.status(), StatusCode::OK);
    for (policy, text) in [(r#"{"enabled":true}"#, "Account disconnected")] {
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
            .contains("Account disconnected")
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
        ("GET", "/api/v1/folders", "demo", secret, 200),
        ("GET", "/api/v1/folders", "other", secret, 403),
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

fn key_store() -> (tempfile::TempDir, colecto_core::Database) {
    let dir = tempfile::tempdir().unwrap();
    let database = colecto_core::Database::open(dir.path()).unwrap();
    (dir, database)
}

#[test]
fn key_secrets_are_not_persisted_and_expiry_revocation_survive_restart() {
    use colecto_extension_api::Capability;
    use colecto_host::api_keys::{ApiKeys, KeyInput};
    let (dir, database) = key_store();
    let open = || ApiKeys::open(database.clone());
    let mut keys = open().unwrap();
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
    let persisted =
        String::from_utf8(database.get_record("host", "api-keys", 1).unwrap().unwrap()).unwrap();
    assert!(!persisted.contains(&secret));
    assert!(!persisted.contains(secret.split_once('.').unwrap().1));
    assert!(
        !dir.path().join("api-keys.json").exists(),
        "policy lives in Turso only"
    );
    let public = serde_json::to_string(&keys.list()).unwrap();
    assert!(!public.contains("verifier"));
    assert!(!public.contains("secret"));
    drop(keys);
    let mut keys = open().unwrap();
    assert!(keys.authenticate(&secret, 199).is_ok());
    assert!(keys.allows(&metadata.id, "demo", Capability::MessagesList, 199));
    assert!(!keys.allows(&metadata.id, "new-account", Capability::MessagesList, 199));
    assert!(keys.authenticate(&secret, 200).is_err());
    keys.revoke(&metadata.id, 199).unwrap();
    drop(keys);
    let mut keys = open().unwrap();
    assert!(keys.authenticate(&secret, 199).is_err());
    assert_eq!(keys.list()[0].last_used_at, Some(199));
}

#[test]
fn missing_and_corrupt_policy_reject_existing_key_and_recent_use_does_not_rewrite_policy() {
    use colecto_extension_api::Capability;
    use colecto_host::api_keys::{ApiKeys, KeyInput};
    let (_dir, database) = key_store();
    let mut keys = ApiKeys::open(database.clone()).unwrap();
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
    let record = || database.get_record("host", "api-keys", 1).unwrap();
    let policy = record().unwrap();
    keys.authenticate(&secret, 101).unwrap();
    assert_eq!(record().unwrap(), policy);
    assert_eq!(keys.list()[0].last_used_at, Some(100));
    database
        .set_record("host", "api-keys", 1, b"corrupt")
        .unwrap();
    assert!(keys.authenticate(&secret, 102).is_err());
    database.set_record("host", "api-keys", 1, &policy).unwrap();
    assert!(keys.authenticate(&secret, 102).is_ok());
    database.remove_record("host", "api-keys", 1).unwrap();
    assert!(keys.authenticate(&secret, 103).is_err());
}

#[test]
fn stale_policy_instances_cannot_resurrect_a_revoked_key() {
    use colecto_extension_api::Capability;
    use colecto_host::api_keys::{ApiKeys, KeyInput};
    let (_dir, database) = key_store();
    let input = || KeyInput {
        name: "Assistant".into(),
        accounts: vec!["demo".into()],
        capabilities: vec![Capability::MessagesList],
        expires_at: None,
    };
    let open = || ApiKeys::open(database.clone()).unwrap();
    let mut first = open();
    let (key, secret) = first.mint(input(), 100).unwrap();
    let mut second = open();
    second.revoke(&key.id, 101).unwrap();
    first.mint(input(), 102).unwrap();
    assert!(first.authenticate(&secret, 103).is_err());
    assert!(open().authenticate(&secret, 103).is_err());
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

#[tokio::test]
async fn scoped_list_responses_require_read_grants_for_message_and_draft_bodies() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = MailStore::open(dir.path()).unwrap();
    store
        .save_draft(colecto_extension_api::DraftInput {
            id: None,
            recipient: "a@example.test".into(),
            subject: "Metadata".into(),
            body: "Private draft body".into(),
            ..Default::default()
        })
        .unwrap();
    let router = app(store, "127.0.0.1:4173".into(), dir.path().into());
    let owner = owner_bearer(&router).await;
    for reads in [false, true] {
        let capabilities = if reads {
            r#"["messages.list","messages.read","drafts.list","drafts.read"]"#
        } else {
            r#"["messages.list","drafts.list"]"#
        };
        let mut mint = request(
            "POST",
            "/api/v1/api-keys",
            "host",
            "http://127.0.0.1:4173",
            &format!(r#"{{"name":"List test","accounts":["demo"],"capabilities":{capabilities}}}"#),
        );
        mint.headers_mut()
            .insert("authorization", owner.parse().unwrap());
        let response = router.clone().oneshot(mint).await.unwrap();
        let minted: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
        let token = format!("Bearer {}", minted["secret"].as_str().unwrap());
        for (method, path) in [
            ("GET", "/api/v1/messages"),
            ("GET", "/api/v1/messages/cache"),
            ("POST", "/api/v1/messages/refresh"),
            ("GET", "/api/v1/drafts"),
        ] {
            let mut req = request(method, path, "host", "http://127.0.0.1:4173", "{}");
            req.headers_mut()
                .insert("authorization", token.parse().unwrap());
            let response = router.clone().oneshot(req).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{path}");
            let body: serde_json::Value =
                serde_json::from_slice(&to_bytes(response.into_body(), 1_000_000).await.unwrap())
                    .unwrap();
            let list = if path.contains("cache") {
                &body["messages"]
            } else {
                &body
            };
            assert!(!list.as_array().unwrap().is_empty());
            assert_eq!(
                list[0]["body"].as_str().unwrap().is_empty(),
                !reads,
                "{path}"
            );
        }
    }
}

#[tokio::test]
async fn owner_vault_lock_clears_cached_folder_and_reader_access() {
    let dir = tempfile::tempdir().unwrap();
    let router = app(
        MailStore::open(dir.path()).unwrap(),
        "127.0.0.1:4173".into(),
        dir.path().into(),
    );
    let bearer = owner_bearer(&router).await;
    let mut cached = request(
        "GET",
        "/api/v1/messages/cache",
        "inbox",
        "http://127.0.0.1:4173",
        "",
    );
    cached
        .headers_mut()
        .insert("authorization", bearer.parse().unwrap());
    let response = router.clone().oneshot(cached).await.unwrap();
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    let id = body["messages"][0]["id"].as_str().unwrap();
    for input in [
        r#"{"action":"protect","password":"a strong master password"}"#,
        r#"{"action":"lock","password":""}"#,
    ] {
        let mut req = request(
            "POST",
            "/api/v1/security",
            "host",
            "http://127.0.0.1:4173",
            input,
        );
        req.headers_mut()
            .insert("authorization", bearer.parse().unwrap());
        assert_eq!(
            router.clone().oneshot(req).await.unwrap().status(),
            StatusCode::OK
        );
    }
    let mut cached = request(
        "GET",
        "/api/v1/messages/cache",
        "inbox",
        "http://127.0.0.1:4173",
        "",
    );
    cached
        .headers_mut()
        .insert("authorization", bearer.parse().unwrap());
    let response = router.clone().oneshot(cached).await.unwrap();
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    assert!(body["messages"].as_array().unwrap().is_empty());
    let mut read = request(
        "GET",
        &format!("/api/v1/messages/{id}"),
        "reader",
        "http://127.0.0.1:4173",
        "",
    );
    read.headers_mut()
        .insert("authorization", bearer.parse().unwrap());
    assert_eq!(
        router.oneshot(read).await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn mcp_answers_only_after_the_owner_turns_it_on_and_preferences_are_owner_only() {
    let dir = tempfile::tempdir().unwrap();
    let router = app(
        MailStore::open(dir.path()).unwrap(),
        "127.0.0.1:4173".into(),
        dir.path().into(),
    );
    let router = authenticated(router).await;
    let mcp = || {
        let mut request = request("POST", "/api/v1/mcp", "host", "http://127.0.0.1:4173", "{}");
        request.headers_mut().insert(
            "accept",
            "application/json, text/event-stream".parse().unwrap(),
        );
        request
    };
    assert_eq!(
        router.clone().oneshot(mcp()).await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
    let set = |extension: &str| {
        request(
            "POST",
            "/api/v1/preferences",
            extension,
            "http://127.0.0.1:4173",
            r#"{"mcp":true}"#,
        )
    };
    assert_eq!(
        router
            .clone()
            .oneshot(set("reader"))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        router.clone().oneshot(set("host")).await.unwrap().status(),
        StatusCode::OK
    );
    assert_ne!(
        router.clone().oneshot(mcp()).await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
    let read = router
        .oneshot(request(
            "GET",
            "/api/v1/preferences",
            "host",
            "http://127.0.0.1:4173",
            "",
        ))
        .await
        .unwrap();
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(read.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(body["mcp"], true);
    assert!(!dir.path().join("preferences.json").exists());
}

#[tokio::test]
async fn assets_require_owner_host_identity_and_enabled_package() {
    let dir = tempfile::tempdir().unwrap();
    let router = authenticated(app(
        MailStore::open(dir.path()).unwrap(),
        "127.0.0.1:4173".into(),
        dir.path().into(),
    ))
    .await;
    let origin = "http://127.0.0.1:4173";
    let package = include_bytes!("../../../docs/extensions/examples/oneko.colectoext");
    let response = router
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/extensions",
            "host",
            origin,
            &serde_json::json!({"package":package.as_slice()}).to_string(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    // The SDK-built guest must run in the real sandbox: within fuel, reading its
    // settings, and producing overlay output the Host accepts.
    let frame = serde_json::json!({"action": serde_json::json!({"event":"frame","time_ms":0,"pointer":{"x":400,"y":16,"inside":true},"viewport":{"width":800,"height":600},"state":""}).to_string()});
    let response = router
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/extensions/oneko/run",
            "host",
            origin,
            &frame.to_string(),
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let output: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
    let item = &output["overlay"]["items"][0];
    assert_eq!(
        (item["image"].as_str(), item["x"].as_f64()),
        (Some("oneko.png"), Some(10.0))
    );
    assert!(output["state"].as_str().unwrap().contains("\"tick\":1.0"));
    let path = "/api/v1/extensions/oneko/assets/oneko.png";
    let response = router
        .clone()
        .oneshot(request("GET", path, "host", origin, ""))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "image/png");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    assert_eq!(
        response.headers()["content-security-policy"],
        "sandbox; default-src 'none'"
    );
    let response = router
        .clone()
        .oneshot(request("GET", path, "reader", origin, ""))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let response = router
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/extensions/oneko",
            "host",
            origin,
            r#"{"enabled":false}"#,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = router
        .oneshot(request("GET", path, "host", origin, ""))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn binary_package_uploads_exceed_the_default_json_body_limit() {
    let dir = tempfile::tempdir().unwrap();
    let router = authenticated(app(
        MailStore::open(dir.path()).unwrap(),
        "127.0.0.1:4173".into(),
        dir.path().into(),
    ))
    .await;
    let manifest =
        include_str!("../../../docs/extensions/examples/connection-panel/extension.toml");
    // A large custom section is valid Wasm and exercises upload/storage without expensive guest work.
    let mut module = wat::parse_str("(module (func (export \"run\")))").unwrap();
    let size = 1024 * 1024 + 1;
    module.push(0);
    let mut remaining = size;
    loop {
        let byte = (remaining & 127) as u8;
        remaining >>= 7;
        module.push(byte | if remaining > 0 { 128 } else { 0 });
        if remaining == 0 {
            break;
        }
    }
    module.push(0);
    module.resize(module.len() + size - 1, 0);
    // Store the ZIP rather than deflating this intentionally repetitive custom section.
    let mut archive = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    use std::io::Write;
    for (name, data) in [
        ("manifest.toml", manifest.as_bytes()),
        ("module.wasm", module.as_slice()),
    ] {
        archive
            .start_file(
                name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
        archive.write_all(data).unwrap();
    }
    let package = archive.finish().unwrap().into_inner();
    let mut upload = request(
        "POST",
        "/api/v1/extensions",
        "host",
        "http://127.0.0.1:4173",
        "",
    );
    upload
        .headers_mut()
        .insert("content-type", "application/octet-stream".parse().unwrap());
    *upload.body_mut() = Body::from(package);
    let response = router.clone().oneshot(upload).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = router
        .oneshot(request(
            "GET",
            "/api/v1/extensions",
            "host",
            "http://127.0.0.1:4173",
            "",
        ))
        .await
        .unwrap();
    let list = to_bytes(response.into_body(), 65536).await.unwrap();
    assert!(list.len() < 8192);
}

#[tokio::test]
async fn folders_require_message_list_permission() {
    let dir = tempfile::tempdir().unwrap();
    let router = authenticated(app(
        MailStore::open(dir.path()).unwrap(),
        "127.0.0.1:4173".into(),
        dir.path().into(),
    ))
    .await;
    for (extension, status) in [
        ("inbox", StatusCode::OK),
        ("composer", StatusCode::FORBIDDEN),
    ] {
        let response = router
            .clone()
            .oneshot(request(
                "GET",
                "/api/v1/folders",
                extension,
                "http://127.0.0.1:4173",
                "",
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        if status == StatusCode::OK {
            let folders: serde_json::Value =
                serde_json::from_slice(&to_bytes(response.into_body(), 200_000).await.unwrap())
                    .unwrap();
            assert_eq!(folders[0]["role"], "inbox");
            assert_eq!(folders[0]["total"], 4);
        }
    }
}

#[tokio::test]
async fn organize_requires_permission_and_bounded_ids() {
    let dir = tempfile::tempdir().unwrap();
    let router = authenticated(app(
        MailStore::open(dir.path()).unwrap(),
        "127.0.0.1:4173".into(),
        dir.path().into(),
    ))
    .await;
    for action in ["flags", "move", "archive", "delete"] {
        let path = format!("/api/v1/messages/{action}");
        for (extension, body, expected) in [
            (
                "composer",
                serde_json::json!({"ids":["welcome"]}),
                StatusCode::FORBIDDEN,
            ),
            (
                "inbox",
                serde_json::json!({"ids":[]}),
                StatusCode::BAD_REQUEST,
            ),
            (
                "inbox",
                serde_json::json!({"ids":vec!["welcome";501]}),
                StatusCode::BAD_REQUEST,
            ),
        ] {
            let response = router
                .clone()
                .oneshot(request(
                    "POST",
                    &path,
                    extension,
                    "http://127.0.0.1:4173",
                    &body.to_string(),
                ))
                .await
                .unwrap();
            assert_eq!(response.status(), expected, "{action}: {extension}");
        }
    }
}

#[tokio::test]
async fn removal_is_host_only_and_clears_selected_account() {
    let dir = tempfile::tempdir().unwrap();
    let router = authenticated(app(
        MailStore::open(dir.path()).unwrap(),
        "127.0.0.1:4173".into(),
        dir.path().into(),
    ))
    .await;
    let call = |method: &str, path: &str, extension: &str, body: &str| {
        request(method, path, extension, "http://127.0.0.1:4173", body)
    };
    assert_eq!(
        router
            .clone()
            .oneshot(call(
                "POST",
                "/api/v1/security",
                "host",
                r#"{"action":"protect","password":"a strong master password"}"#
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let response = router.clone().oneshot(call("POST", "/api/v1/accounts", "host", r#"{"name":"Work","address":"work@example.test","protocol":"imap","imap_host":"imap.example.test","smtp_host":"smtp.example.test","username":"work","secret":"secret"}"#)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let info: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 200000).await.unwrap()).unwrap();
    let id = info["id"].as_str().unwrap();
    assert_eq!(
        router
            .clone()
            .oneshot(call(
                "POST",
                "/api/v1/preferences",
                "host",
                &format!(r#"{{"account":"{id}"}}"#)
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let path = format!("/api/v1/accounts/{id}");
    assert_eq!(
        router
            .clone()
            .oneshot(call("DELETE", &path, "inbox", ""))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    let response = router
        .clone()
        .oneshot(call("DELETE", &path, "host", ""))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        to_bytes(response.into_body(), 200000)
            .await
            .unwrap()
            .as_ref(),
        b"true"
    );
    let response = router
        .clone()
        .oneshot(call("GET", "/api/v1/accounts", "host", ""))
        .await
        .unwrap();
    let accounts: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 200000).await.unwrap()).unwrap();
    assert!(!accounts.as_array().unwrap().iter().any(|a| a["id"] == id));
    let response = router
        .clone()
        .oneshot(call("GET", "/api/v1/preferences", "host", ""))
        .await
        .unwrap();
    let prefs: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 200000).await.unwrap()).unwrap();
    assert!(prefs["account"].is_null());
    assert_eq!(
        router
            .oneshot(call("DELETE", "/api/v1/accounts/demo", "host", ""))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn version_is_public_but_keeps_the_host_boundary() {
    let dir = tempfile::tempdir().unwrap();
    let router = app(
        MailStore::open(dir.path()).unwrap(),
        "127.0.0.1:4173".into(),
        dir.path().into(),
    );
    let get = |host: &str| {
        Request::builder()
            .uri("/api/v1/version")
            .header("host", host)
            .body(Body::empty())
            .unwrap()
    };
    let response = router.clone().oneshot(get("127.0.0.1:4173")).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(body["name"], "colecto");
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(body["api"], "v1");
    assert!(body["commit"].is_null() || body["commit"].is_string());
    let forged = router.oneshot(get("evil.example")).await.unwrap();
    assert_eq!(forged.status(), StatusCode::FORBIDDEN);
}
