use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use colecto_core::{Capability, Database, DraftInput, MailStore};
use colecto_daemon::{
    api_keys::{ApiKeys, KeyInput},
    app as daemon_app,
};
use tower::ServiceExt;

fn owner_token() -> String {
    "a".repeat(64)
}

fn app(store: MailStore, authority: String, assets: std::path::PathBuf) -> axum::Router {
    daemon_app(store, authority, owner_token(), assets.join("assets"))
}

fn request(method: &str, path: &str, origin: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:4173")
        .header("origin", origin)
        .header("x-colecto-account", "demo")
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

async fn authenticated(router: axum::Router) -> axum::Router {
    let bearer = owner_bearer(&router).await;
    router.layer(axum::middleware::from_fn(
        move |mut req: axum::extract::Request, next: axum::middleware::Next| {
            if !req.headers().contains_key("authorization") {
                req.headers_mut()
                    .insert("authorization", bearer.parse().unwrap());
            }
            async move { next.run(req).await }
        },
    ))
}

async fn owner_bearer(router: &axum::Router) -> String {
    let bearer = format!("Bearer {}", owner_token());
    let mut validate = request("POST", "/api/v1/session", "http://127.0.0.1:4173", "{}");
    validate
        .headers_mut()
        .insert("authorization", bearer.parse().unwrap());
    let response = router.clone().oneshot(validate).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(!response.headers().contains_key("set-cookie"));
    bearer
}

#[tokio::test]
async fn granted_save_and_read_share_core_state_without_sidecar_files() {
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
        .clone()
        .oneshot(request(
            "GET",
            &format!("/api/v1/drafts/{id}"),
            "http://127.0.0.1:4173",
            "",
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let draft: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 200_000).await.unwrap()).unwrap();
    assert_eq!(draft["subject"], "Saved");

    // Extensions API does not exist in Colecto.
    let ext = router
        .oneshot(request(
            "GET",
            "/api/v1/extensions",
            "http://127.0.0.1:4173",
            "",
        ))
        .await
        .unwrap();
    assert_eq!(ext.status(), StatusCode::NOT_FOUND);

    // No sidecar files are created alongside colecto.db.
    assert!(dir.path().join("colecto.db").exists());
    for forbidden in [
        "owner-token",
        "daemon.json",
        "colecto-settings.json",
        "colecto-settings.json",
        "preferences.json",
        "api-keys.json",
    ] {
        assert!(
            !dir.path().join(forbidden).exists(),
            "{forbidden} must not exist"
        );
    }
}

#[tokio::test]
async fn daemon_rejects_cross_origin_ungranted_and_malformed_requests() {
    let dir = tempfile::tempdir().unwrap();
    let router = app(
        MailStore::open(dir.path()).unwrap(),
        "127.0.0.1:4173".into(),
        dir.path().into(),
    );
    let router = authenticated(router).await;
    for (method, path, origin, body, expected) in [
        ("POST", "/api/v1/drafts", "https://evil.test", "{}", 403),
        ("POST", "/api/v1/drafts", "", "{}", 403),
        (
            "GET",
            "/api/v1/messages/missing",
            "http://127.0.0.1:4173",
            "",
            404,
        ),
        ("POST", "/api/v1/drafts", "http://127.0.0.1:4173", "{}", 400),
        (
            "POST",
            "/api/v1/drafts",
            "http://127.0.0.1:4173",
            r#"{"id":"missing","recipient":"","subject":"","body":""}"#,
            404,
        ),
    ] {
        let response = router
            .clone()
            .oneshot(request(method, path, origin, body))
            .await
            .unwrap();
        assert_eq!(
            response.status().as_u16(),
            expected,
            "{method} {path} {origin}"
        );
        let error: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 200_000).await.unwrap())
                .unwrap();
        assert!(error["error"].is_string());
    }
    let mut req = request("GET", "/api/v1/messages", "http://127.0.0.1:4173", "");
    req.headers_mut()
        .insert("host", "evil.test".parse().unwrap());
    assert_eq!(
        router.clone().oneshot(req).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let mut req = request("GET", "/api/v1/messages", "http://127.0.0.1:4173", "");
    req.headers_mut()
        .insert("x-colecto-account", "other".parse().unwrap());
    assert_eq!(
        router.oneshot(req).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
}

#[tokio::test]
async fn send_content_conversation_and_attachment_sized_bodies() {
    let dir = tempfile::tempdir().unwrap();
    let router = app(
        MailStore::open(dir.path()).unwrap(),
        "127.0.0.1:4173".into(),
        dir.path().into(),
    );
    let router = authenticated(router).await;
    let origin = "http://127.0.0.1:4173";

    let content = router
        .clone()
        .oneshot(request(
            "GET",
            "/api/v1/messages/welcome/content?remote_images=false",
            origin,
            "",
        ))
        .await
        .unwrap();
    assert_eq!(content.status(), StatusCode::OK);

    let conv = router
        .clone()
        .oneshot(request(
            "GET",
            "/api/v1/messages/welcome/conversation",
            origin,
            "",
        ))
        .await
        .unwrap();
    assert_eq!(conv.status(), StatusCode::OK);

    let attachment = "A".repeat(2_000_000);
    let body = format!(
        r#"{{"recipient":"a@example.test","subject":"Hi","body":"x","attachments":[{{"name":"a.bin","content_type":"application/octet-stream","data":"{attachment}"}}]}}"#
    );
    let response = router
        .clone()
        .oneshot(request("POST", "/api/v1/send", origin, &body))
        .await
        .unwrap();
    // Reaches the demo store (no account) instead of the global 512 KB body limit.
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);

    let account = router
        .oneshot(request("GET", "/api/v1/account", origin, ""))
        .await
        .unwrap();
    assert_eq!(
        to_bytes(account.into_body(), 100).await.unwrap().as_ref(),
        b"false"
    );
}

#[tokio::test]
async fn api_keys_enforce_capabilities_and_accounts() {
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
                .oneshot(request("GET", path, "http://127.0.0.1:4173", ""))
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
    assert!(secret.starts_with("colecto_"));
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
        let mut req = request(method, path, "http://127.0.0.1:4173", "{}");
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
    let mut req = request("GET", "/api/v1/messages", "http://127.0.0.1:4173", "");
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
        let mut req = request("POST", "/api/v1/session", "http://127.0.0.1:4173", "{}");
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
    let dir = tempfile::tempdir().unwrap();
    let database = Database::open(dir.path()).unwrap();
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
    assert!(secret.starts_with("colecto_"));
    let persisted = database
        .query(
            "SELECT id||name||accounts||capabilities||hex(verifier) FROM api_keys",
            vec![],
        )
        .unwrap()[0]
        .text(0)
        .unwrap();
    assert!(!persisted.contains(&secret));
    assert!(!persisted.contains(secret.split_once('.').unwrap().1));
    assert!(!dir.path().join("api-keys.json").exists());
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

#[tokio::test]
async fn scoped_list_responses_require_read_grants_for_message_and_draft_bodies() {
    let dir = tempfile::tempdir().unwrap();
    let mut store = MailStore::open(dir.path()).unwrap();
    store
        .save_draft(DraftInput {
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
            let mut req = request(method, path, "http://127.0.0.1:4173", "{}");
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
async fn mcp_answers_only_after_owner_turns_it_on_and_version_is_public() {
    let dir = tempfile::tempdir().unwrap();
    let raw_router = app(
        MailStore::open(dir.path()).unwrap(),
        "127.0.0.1:4173".into(),
        dir.path().into(),
    );
    let version = raw_router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/version")
                .header("host", "127.0.0.1:4173")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(version.status(), StatusCode::OK);
    let body: serde_json::Value =
        serde_json::from_slice(&to_bytes(version.into_body(), 4096).await.unwrap()).unwrap();
    assert_eq!(body["name"], "colecto");
    assert_eq!(body["api"], "v1");

    let router = authenticated(raw_router).await;
    let mcp = || {
        let mut req = request("POST", "/api/v1/mcp", "http://127.0.0.1:4173", "{}");
        req.headers_mut().insert(
            "accept",
            "application/json, text/event-stream".parse().unwrap(),
        );
        req
    };
    assert_eq!(
        router.clone().oneshot(mcp()).await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        router
            .clone()
            .oneshot(request(
                "POST",
                "/api/v1/preferences",
                "http://127.0.0.1:4173",
                r#"{"mcp":true}"#,
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_ne!(
        router.clone().oneshot(mcp()).await.unwrap().status(),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn organize_and_account_removal_work_and_clear_selected_account() {
    let dir = tempfile::tempdir().unwrap();
    let router = authenticated(app(
        MailStore::open(dir.path()).unwrap(),
        "127.0.0.1:4173".into(),
        dir.path().into(),
    ))
    .await;
    let origin = "http://127.0.0.1:4173";
    for action in ["flags", "move", "archive", "delete"] {
        let path = format!("/api/v1/messages/{action}");
        for body in [
            serde_json::json!({"ids":[]}),
            serde_json::json!({"ids":vec!["welcome"; 501]}),
        ] {
            let response = router
                .clone()
                .oneshot(request("POST", &path, origin, &body.to_string()))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{action}");
        }
    }
    assert_eq!(
        router
            .clone()
            .oneshot(request(
                "POST",
                "/api/v1/security",
                origin,
                r#"{"action":"protect","password":"a strong master password"}"#,
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let response = router
        .clone()
        .oneshot(request(
            "POST",
            "/api/v1/accounts",
            origin,
            r#"{"name":"Work","address":"work@example.test","protocol":"imap","imap_host":"imap.example.test","smtp_host":"smtp.example.test","username":"work","secret":"never-return-this"}"#,
        ))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = to_bytes(response.into_body(), 200_000).await.unwrap();
    assert!(!String::from_utf8_lossy(&bytes).contains("never-return-this"));
    let info: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let id = info["id"].as_str().unwrap();
    assert_eq!(
        router
            .clone()
            .oneshot(request(
                "POST",
                "/api/v1/preferences",
                origin,
                &format!(r#"{{"account":"{id}"}}"#),
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        router
            .clone()
            .oneshot(request(
                "DELETE",
                &format!("/api/v1/accounts/{id}"),
                origin,
                ""
            ))
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    let prefs_res = router
        .clone()
        .oneshot(request("GET", "/api/v1/preferences", origin, ""))
        .await
        .unwrap();
    let prefs: serde_json::Value =
        serde_json::from_slice(&to_bytes(prefs_res.into_body(), 200_000).await.unwrap()).unwrap();
    assert!(prefs["account"].is_null());
    assert_eq!(
        router
            .oneshot(request("DELETE", "/api/v1/accounts/demo", origin, ""))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn localhost_alias_and_bearer_without_origin_work_for_local_and_native_clients() {
    let dir = tempfile::tempdir().unwrap();
    let raw_router = app(
        MailStore::open(dir.path()).unwrap(),
        "127.0.0.1:4173".into(),
        dir.path().into(),
    );
    let bearer = owner_bearer(&raw_router).await;

    // Localhost host + origin alias works when bound to 127.0.0.1:<port>.
    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/drafts")
        .header("host", "localhost:4173")
        .header("origin", "http://localhost:4173")
        .header("authorization", &bearer)
        .header("x-colecto-account", "demo")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"recipient":"a@example.test","subject":"Localhost","body":"Hi"}"#,
        ))
        .unwrap();
    assert_eq!(
        raw_router.clone().oneshot(req).await.unwrap().status(),
        StatusCode::OK
    );

    // Wrong port on localhost is still rejected.
    let bad_port = Request::builder()
        .method("GET")
        .uri("/api/v1/messages")
        .header("host", "localhost:9999")
        .header("authorization", &bearer)
        .header("x-colecto-account", "demo")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        raw_router.clone().oneshot(bad_port).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );

    // Native client (e.g. RemoteHttpBackend) mutation without Origin succeeds with a valid Bearer.
    let no_origin_ok = Request::builder()
        .method("POST")
        .uri("/api/v1/drafts")
        .header("host", "127.0.0.1:4173")
        .header("authorization", &bearer)
        .header("x-colecto-account", "demo")
        .header("content-type", "application/json")
        .body(Body::from(
            r#"{"recipient":"b@example.test","subject":"Native","body":"Hi"}"#,
        ))
        .unwrap();
    assert_eq!(
        raw_router
            .clone()
            .oneshot(no_origin_ok)
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );

    // Missing Origin with an invalid Bearer still gets 401.
    let no_origin_bad_bearer = Request::builder()
        .method("POST")
        .uri("/api/v1/drafts")
        .header("host", "127.0.0.1:4173")
        .header("authorization", "Bearer invalid")
        .header("x-colecto-account", "demo")
        .header("content-type", "application/json")
        .body(Body::from("{}"))
        .unwrap();
    assert_eq!(
        raw_router
            .oneshot(no_origin_bad_bearer)
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn sse_events_stream_live_updates_and_reject_unauthorized_clients() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let dir = tempfile::tempdir().unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let authority = addr.to_string();
    let router = app(
        MailStore::open(dir.path()).unwrap(),
        authority.clone(),
        dir.path().into(),
    );
    let bearer = format!("Bearer {}", owner_token());

    let server_router = router.clone();
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, server_router).await;
    });

    // Unauthenticated SSE stream must be rejected with 401.
    let mut unauth = tokio::net::TcpStream::connect(addr).await.unwrap();
    unauth
        .write_all(
            format!(
                "GET /api/v1/events HTTP/1.1\r\nHost: {authority}\r\nAccept: text/event-stream\r\nConnection: close\r\n\r\n"
            )
            .as_bytes(),
        )
        .await
        .unwrap();
    let mut unauth_buf = vec![0u8; 256];
    let n = unauth.read(&mut unauth_buf).await.unwrap();
    assert!(
        String::from_utf8_lossy(&unauth_buf[..n]).starts_with("HTTP/1.1 401"),
        "expected 401 for unauthenticated SSE"
    );

    // Authenticated SSE stream emits `ready`, then live `preferences` and `labels` events.
    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(
            format!(
                "GET /api/v1/events HTTP/1.1\r\nHost: {authority}\r\nAuthorization: {bearer}\r\nAccept: text/event-stream\r\n\r\n"
            )
            .as_bytes(),
        )
        .await
        .unwrap();

    let mut received = String::new();
    let mut chunk = [0u8; 2048];
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    while !received.contains("event: ready") {
        let n = tokio::time::timeout_at(deadline, stream.read(&mut chunk))
            .await
            .expect("timed out waiting for SSE ready event")
            .unwrap();
        assert!(n > 0, "SSE stream closed before ready event");
        received.push_str(&String::from_utf8_lossy(&chunk[..n]));
    }

    // Trigger preferences + MCP mutations through the same daemon router.
    let pref_req = Request::builder()
        .method("POST")
        .uri("/api/v1/preferences")
        .header("host", &authority)
        .header("origin", format!("http://{authority}"))
        .header("authorization", &bearer)
        .header("content-type", "application/json")
        .body(Body::from(r#"{"list_density":"compact","mcp":true}"#))
        .unwrap();
    assert_eq!(
        router.oneshot(pref_req).await.unwrap().status(),
        StatusCode::OK
    );

    while !received.contains("event: mcp") {
        let n = tokio::time::timeout_at(deadline, stream.read(&mut chunk))
            .await
            .expect("timed out waiting for live SSE mcp event")
            .unwrap();
        assert!(n > 0, "SSE stream closed before live events arrived");
        received.push_str(&String::from_utf8_lossy(&chunk[..n]));
    }
    assert!(received.contains(r#""on":true"#), "{received}");

    server.abort();
}
