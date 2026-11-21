use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use colecto_core::{
    Accounts, Capability, Database, Error, MailStore, SecretStore, Value as DbValue,
};
use colecto_daemon::{
    AccessPolicy, Daemon, MailService,
    api_keys::{ApiKeys, KeyInput, now},
    approvals::Approvals,
    events::ServiceEvent,
};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tower::ServiceExt;

#[derive(Default)]
struct Secrets(Mutex<BTreeMap<String, String>>);
impl SecretStore for Secrets {
    fn get(&self, id: &str) -> Result<String, Error> {
        self.0
            .lock()
            .unwrap()
            .get(id)
            .cloned()
            .ok_or(Error::Missing)
    }
    fn set(&self, id: &str, secret: &str) -> Result<(), Error> {
        self.0.lock().unwrap().insert(id.into(), secret.into());
        Ok(())
    }
    fn delete(&self, id: &str) -> Result<(), Error> {
        self.0.lock().unwrap().remove(id);
        Ok(())
    }
}

fn edit_key(database: &Database, id: &str, column: &str, value: DbValue) {
    database
        .write(vec![(
            &format!("UPDATE api_keys SET {column}=?2 WHERE id=?1"),
            vec![id.into(), value],
        )])
        .unwrap();
}

struct Fixture {
    dir: tempfile::TempDir,
    backend: Arc<Secrets>,
    owner_token: String,
    account: String,
    key: String,
    secret: String,
    other: String,
}

impl Fixture {
    fn new(port: u16) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let backend = Arc::new(Secrets::default());
        let mut accounts =
            Accounts::open(MailStore::open(dir.path()).unwrap(), backend.clone()).unwrap();
        let account = accounts
            .upsert(
                serde_json::from_value(json!({
                    "address": "from@example.test",
                    "protocol": "jmap",
                    "imap_host": "localhost",
                    "smtp_host": "localhost",
                    "smtp_port": 587,
                    "session_url": format!("https://127.0.0.1:{port}/session"),
                    "username": "from",
                    "secret": "fixture-secret"
                }))
                .unwrap(),
            )
            .unwrap()
            .id;
        let database = accounts.database();
        let mut keys = ApiKeys::open(database.clone()).unwrap();
        database
            .write_settings("core", &[("mcp".into(), Some("true".into()))])
            .unwrap();
        let input = || KeyInput {
            name: "Fixture".into(),
            accounts: vec![account.clone(), "demo".into()],
            capabilities: vec![
                Capability::MessagesSend,
                Capability::MessagesList,
                Capability::AppNavigate,
                Capability::SettingsAppearance,
                Capability::SettingsMail,
            ],
            expires_at: None,
        };
        let (key, secret) = keys.mint(input(), now()).unwrap();
        let (_, other) = keys.mint(input(), now()).unwrap();
        Self {
            dir,
            backend,
            owner_token: "a".repeat(64),
            account,
            key: key.id,
            secret,
            other,
        }
    }

    fn daemon(&self, authority: &str) -> Daemon {
        let accounts = Accounts::open(
            MailStore::open(self.dir.path()).unwrap(),
            self.backend.clone(),
        )
        .unwrap();
        let service = MailService::new(accounts).unwrap();
        let policy = AccessPolicy::local(authority.into(), self.owner_token.clone()).unwrap();
        Daemon::new(service, policy, self.dir.path().join("assets")).unwrap()
    }

    fn req(&self, token: &str, method: &str, path: &str, input: Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(format!("/api/v1{path}"))
            .header("Host", "127.0.0.1:4173")
            .header("Origin", "http://127.0.0.1:4173")
            .header("Authorization", format!("Bearer {token}"))
            .header("x-colecto-account", &self.account)
            .header("Content-Type", "application/json")
            .body(Body::from(input.to_string()))
            .unwrap()
    }
}

async fn response(router: axum::Router, req: Request<Body>) -> (u16, Value) {
    let response = router.oneshot(req).await.unwrap();
    let status = response.status().as_u16();
    let value = serde_json::from_slice(&to_bytes(response.into_body(), 2_000_000).await.unwrap())
        .unwrap_or(Value::Null);
    (status, value)
}

fn message() -> Value {
    json!({"recipient":"to@example.test","subject":"Exact snapshot","body":"Original body"})
}

fn message_with_id() -> Value {
    let mut message = message();
    message["id"] = Value::Null;
    message
}

#[tokio::test]
async fn scoped_request_is_immutable_owner_only_and_one_attempt_across_restart() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let fixture = Fixture::new(listener.local_addr().unwrap().port());
    let daemon = fixture.daemon("127.0.0.1:4173");
    let router = daemon.router();
    let (status, pending) = response(
        router.clone(),
        fixture.req(&fixture.secret, "POST", "/send", message()),
    )
    .await;
    assert_eq!(status, 200);
    assert_eq!(pending["status"], "pending");
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(30), listener.accept())
            .await
            .is_err()
    );
    let id = pending["id"].as_str().unwrap();
    assert_eq!(
        response(
            router.clone(),
            fixture.req(
                &fixture.other,
                "GET",
                &format!("/send-requests/{id}"),
                json!({})
            )
        )
        .await
        .0,
        404
    );
    for expected in [200, 429] {
        assert_eq!(
            response(
                router.clone(),
                fixture.req(
                    &fixture.secret,
                    "GET",
                    &format!("/send-requests/{id}"),
                    json!({})
                )
            )
            .await
            .0,
            expected
        );
    }
    for decision in ["approve", "reject"] {
        assert_eq!(
            response(
                router.clone(),
                fixture.req(
                    &fixture.secret,
                    "POST",
                    &format!("/send-requests/{id}/{decision}"),
                    json!({"approved":true})
                )
            )
            .await
            .0,
            403
        );
    }
    let (_, list) = response(
        router.clone(),
        fixture.req(&fixture.owner_token, "GET", "/send-requests", json!({})),
    )
    .await;
    assert_eq!(list[0]["snapshot"], message_with_id());
    let approve = tokio::spawn(router.clone().oneshot(fixture.req(
        &fixture.owner_token,
        "POST",
        &format!("/send-requests/{id}/approve"),
        json!({}),
    )));
    let (socket, _) = tokio::time::timeout(std::time::Duration::from_secs(5), listener.accept())
        .await
        .unwrap()
        .unwrap();
    approve.abort();
    drop(socket);
    let mut final_status = Value::Null;
    for _ in 0..100 {
        let (_, list) = response(
            router.clone(),
            fixture.req(&fixture.owner_token, "GET", "/send-requests", json!({})),
        )
        .await;
        final_status = list[0]["status"].clone();
        if final_status == "uncertain" {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert_eq!(final_status, "uncertain");
    assert_eq!(
        response(
            router.clone(),
            fixture.req(
                &fixture.owner_token,
                "POST",
                &format!("/send-requests/{id}/approve"),
                json!({})
            )
        )
        .await
        .0,
        409
    );
    drop(router);
    drop(daemon);
    let restarted = fixture.daemon("127.0.0.1:4173").router();
    assert_eq!(
        response(
            restarted,
            fixture.req(
                &fixture.owner_token,
                "POST",
                &format!("/send-requests/{id}/approve"),
                json!({})
            )
        )
        .await
        .0,
        409
    );
}

#[tokio::test]
async fn revocation_disconnect_and_expiry_prevent_submission() {
    for reason in [
        "revoked",
        "disconnected",
        "removed",
        "missing-grant",
        "expired",
    ] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let f = Fixture::new(listener.local_addr().unwrap().port());
        let daemon = f.daemon("127.0.0.1:4173");
        let router = daemon.router();
        let (_, pending) =
            response(router.clone(), f.req(&f.secret, "POST", "/send", message())).await;
        let mut id = pending["id"].as_str().unwrap().to_owned();
        match reason {
            "revoked" => ApiKeys::open(daemon.service().database())
                .unwrap()
                .revoke(&f.key, now())
                .unwrap(),
            "removed" => {
                assert_eq!(
                    response(
                        router.clone(),
                        f.req(
                            &f.owner_token,
                            "POST",
                            "/security",
                            json!({"action":"reset"})
                        )
                    )
                    .await
                    .0,
                    200
                );
            }
            "missing-grant" => {
                edit_key(
                    &daemon.service().database(),
                    &f.key,
                    "capabilities",
                    r#"["messages.list"]"#.into(),
                );
            }
            "disconnected" => {
                assert_eq!(
                    response(
                        router.clone(),
                        f.req(
                            &f.owner_token,
                            "POST",
                            &format!("/accounts/{}/disconnect", f.account),
                            json!({})
                        )
                    )
                    .await
                    .0,
                    200
                );
            }
            _ => {
                drop(router);
                drop(daemon);
                let database = Database::open(f.dir.path()).unwrap();
                let mut approvals = Approvals::open(database, now()).unwrap();
                id = approvals
                    .create(
                        &f.key,
                        &f.account,
                        "from@example.test",
                        serde_json::from_value(message()).unwrap(),
                        now() - 601,
                    )
                    .unwrap()
                    .id;
                drop(approvals);
                let router = f.daemon("127.0.0.1:4173").router();
                assert_eq!(
                    response(
                        router,
                        f.req(
                            &f.owner_token,
                            "POST",
                            &format!("/send-requests/{id}/approve"),
                            json!({})
                        )
                    )
                    .await
                    .0,
                    409
                );
                continue;
            }
        }
        assert_eq!(
            response(
                router,
                f.req(
                    &f.owner_token,
                    "POST",
                    &format!("/send-requests/{id}/approve"),
                    json!({})
                )
            )
            .await
            .0,
            403
        );
    }
}

#[tokio::test]
async fn official_sdk_http_and_stdio_interoperate_and_recheck_live_keys() {
    sdk_interop(false).await;
    sdk_interop(true).await;
}

async fn sdk_interop(expire: bool) {
    use rmcp::{
        ServiceExt,
        model::CallToolRequestParams,
        transport::{
            StreamableHttpClientTransport,
            streamable_http_client::StreamableHttpClientTransportConfig,
        },
    };
    let f = Fixture::new(9);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let daemon = f.daemon(&address.to_string());
    let mut events = daemon.service().subscribe();
    let server = tokio::spawn(axum::serve(listener, daemon.router()).into_future());
    let url = format!("http://{address}/api/v1/mcp");
    let mut config = StreamableHttpClientTransportConfig::with_uri(url.clone());
    config.auth_header = Some(f.secret.clone());
    config.allow_stateless = true;
    let http = ().serve(StreamableHttpClientTransport::from_config(config)).await.unwrap();
    assert_eq!(
        http.peer_info().unwrap().protocol_version.as_str(),
        "2025-11-25"
    );
    let tools = http.list_all_tools().await.unwrap();
    assert_eq!(tools.len(), 20);
    assert!(tools.iter().any(|t| t.name == "messages.send"));
    assert!(!tools.iter().any(|t| t.name.contains("approve")));

    let arguments = json!({"account":"demo"}).as_object().unwrap().clone();
    let result = http
        .call_tool(CallToolRequestParams::new("messages.list").with_arguments(arguments.clone()))
        .await
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    assert!(
        result
            .structured_content
            .as_ref()
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .all(|message| message["body"] == "")
    );

    let pending = http
        .call_tool(
            CallToolRequestParams::new("messages.send").with_arguments(
                json!({"account": f.account, "draft": message()})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(pending["status"], "pending");
    match events.recv().await.unwrap() {
        ServiceEvent::Approval { id, subject, .. } => {
            assert_eq!(id, pending["id"].as_str().unwrap());
            assert_eq!(subject, "Exact snapshot");
        }
        other => panic!("unexpected event: {other:?}"),
    }

    let opened = http
        .call_tool(
            CallToolRequestParams::new("app.navigate").with_arguments(
                json!({"account": f.account, "target": "settings/notifications"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    assert!(!opened.is_error.unwrap_or(false));
    match events.recv().await.unwrap() {
        ServiceEvent::Navigate { target, account } => {
            assert_eq!(target, "settings/notifications");
            assert_eq!(account, f.account);
        }
        other => panic!("unexpected event: {other:?}"),
    }

    // Settings: appearance and mail granted, notifications not granted.
    let settings = |name: &str, input: Value| {
        let mut arguments = json!({"account": f.account});
        if !input.is_null() {
            arguments["settings"] = input;
        }
        CallToolRequestParams::new(name.to_owned())
            .with_arguments(arguments.as_object().unwrap().clone())
    };
    let read = http
        .call_tool(settings("settings.read", Value::Null))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert!(read.get("appearance").is_some());
    assert!(read.get("mail").is_some());
    assert!(read.get("notifications").is_none());
    assert!(read.get("extensions").is_none());

    let changed = http
        .call_tool(settings(
            "settings.update",
            json!({
                "appearance": {"accent": "#8EC5F0", "list_density": "compact"},
                "mail": {"remote_images": true, "prefetch": {"count": 20}}
            }),
        ))
        .await
        .unwrap();
    assert!(!changed.is_error.unwrap_or(false));
    let changed = changed.structured_content.unwrap();
    assert_eq!(changed["appearance"]["accent"], "#8ec5f0");
    assert_eq!(changed["appearance"]["list_density"], "compact");
    assert_eq!(changed["mail"]["remote_images"], true);
    assert_eq!(changed["mail"]["prefetch"]["count"], 20);
    assert_eq!(events.recv().await.unwrap(), ServiceEvent::Preferences);

    for refused in [
        json!({"notifications":{"system":true}}),
        json!({"appearance":{"accent":"red"}}),
        json!({"appearance":{"font_size":40}}),
        json!({"appearance":{"text_spacing":{"line":3}}}),
        json!({"mail":{"prefetch":{"count":999}}}),
        json!({"mcp":false}),
    ] {
        let result = http
            .call_tool(settings("settings.update", refused))
            .await
            .unwrap();
        assert_eq!(result.is_error, Some(true));
    }

    let credential = f.dir.path().join("mcp-key");
    std::fs::write(&credential, &f.secret).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&credential, std::fs::Permissions::from_mode(0o600)).unwrap();
    }
    let mut child = tokio::process::Command::new(env!("CARGO_BIN_EXE_colecto-mcp"))
        .arg(&url)
        .env("COLECTO_MCP_KEY_FILE", &credential)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::inherit())
        .kill_on_drop(true)
        .spawn()
        .unwrap();
    let stdio =
        ().serve((child.stdout.take().unwrap(), child.stdin.take().unwrap()))
            .await
            .unwrap();
    assert_eq!(stdio.list_all_tools().await.unwrap().len(), tools.len());
    let result = stdio
        .call_tool(CallToolRequestParams::new("messages.list").with_arguments(arguments.clone()))
        .await
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));

    if expire {
        let expires_at = now() + 1;
        edit_key(
            &daemon.service().database(),
            &f.key,
            "expires_at",
            (expires_at as i64).into(),
        );
        while now() < expires_at {
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    } else {
        ApiKeys::open(daemon.service().database())
            .unwrap()
            .revoke(&f.key, now())
            .unwrap();
    }
    assert!(
        http.call_tool(
            CallToolRequestParams::new("messages.list").with_arguments(arguments.clone())
        )
        .await
        .is_err()
    );
    assert!(
        stdio
            .call_tool(CallToolRequestParams::new("messages.list").with_arguments(arguments))
            .await
            .is_err()
    );
    let _ = http.cancel().await;
    let _ = stdio.cancel().await;
    let _ = child.kill().await;
    server.abort();
}
