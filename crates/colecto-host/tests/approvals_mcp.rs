use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use colecto_core::{Accounts, Error, MailStore, SecretStore};
use colecto_extension_api::Capability;
use colecto_host::{
    AccessPolicy, MailService,
    api_keys::{ApiKeys, KeyInput, now},
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
struct Fixture {
    dir: tempfile::TempDir,
    backend: Arc<Secrets>,
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
        let account = accounts.upsert(serde_json::from_value(json!({"address":"from@example.test","protocol":"jmap","imap_host":"localhost","smtp_host":"localhost","smtp_port":587,"session_url":format!("https://127.0.0.1:{port}/session"),"username":"from","secret":"fixture-secret"})).unwrap()).unwrap().id;
        let mut keys = ApiKeys::open(dir.path()).unwrap();
        let input = || KeyInput {
            name: "Fixture".into(),
            accounts: vec![account.clone(), "demo".into()],
            capabilities: vec![Capability::MessagesSend, Capability::MessagesList],
            expires_at: None,
        };
        let (key, secret) = keys.mint(input(), now()).unwrap();
        let (_, other) = keys.mint(input(), now()).unwrap();
        Self {
            dir,
            backend,
            account,
            key: key.id,
            secret,
            other,
        }
    }
    fn service(&self, authority: &str) -> MailService {
        let accounts = Accounts::open(
            MailStore::open(self.dir.path()).unwrap(),
            self.backend.clone(),
        )
        .unwrap();
        MailService::new(
            accounts,
            AccessPolicy::local(authority.into()),
            self.dir.path().join("assets"),
        )
        .unwrap()
    }
    fn owner(&self) -> String {
        std::fs::read_to_string(self.dir.path().join("owner-token")).unwrap()
    }
    fn req(&self, token: &str, method: &str, path: &str, input: Value) -> Request<Body> {
        Request::builder()
            .method(method)
            .uri(format!("/api/v1{path}"))
            .header("Host", "127.0.0.1:4173")
            .header("Origin", "http://127.0.0.1:4173")
            .header("Authorization", format!("Bearer {token}"))
            .header("x-colecto-extension", "host")
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
#[tokio::test]
async fn scoped_request_is_immutable_owner_only_and_one_attempt_across_restart() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let fixture = Fixture::new(listener.local_addr().unwrap().port());
    let service = fixture.service("127.0.0.1:4173");
    let router = service.router();
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
        fixture.req(&fixture.owner(), "GET", "/send-requests", json!({})),
    )
    .await;
    assert_eq!(list[0]["snapshot"], message_with_id());
    let approve = tokio::spawn(router.clone().oneshot(fixture.req(
        &fixture.owner(),
        "POST",
        &format!("/send-requests/{id}/approve"),
        json!({}),
    )));
    let (socket, _) = tokio::time::timeout(std::time::Duration::from_secs(5), listener.accept())
        .await
        .unwrap()
        .unwrap();
    // Disconnecting the reviewing browser cannot cancel the persisted submission.
    approve.abort();
    drop(socket);
    let mut final_status = Value::Null;
    for _ in 0..100 {
        let (_, list) = response(
            router.clone(),
            fixture.req(&fixture.owner(), "GET", "/send-requests", json!({})),
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
                &fixture.owner(),
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
    drop(service);
    let restarted = fixture.service("127.0.0.1:4173").router();
    assert_eq!(
        response(
            restarted,
            fixture.req(
                &fixture.owner(),
                "POST",
                &format!("/send-requests/{id}/approve"),
                json!({})
            )
        )
        .await
        .0,
        409
    );
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(30), listener.accept())
            .await
            .is_err()
    );
}
fn message_with_id() -> Value {
    let mut message = message();
    message["id"] = Value::Null;
    message
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
        let service = f.service("127.0.0.1:4173");
        let router = service.router();
        let (_, pending) =
            response(router.clone(), f.req(&f.secret, "POST", "/send", message())).await;
        let mut id = pending["id"].as_str().unwrap().to_owned();
        match reason {
            "revoked" => ApiKeys::open(f.dir.path())
                .unwrap()
                .revoke(&f.key, now())
                .unwrap(),
            "removed" => {
                assert_eq!(
                    response(
                        router.clone(),
                        f.req(&f.owner(), "POST", "/security", json!({"action":"reset"}))
                    )
                    .await
                    .0,
                    200
                );
            }
            "missing-grant" => {
                let path = f.dir.path().join("api-keys.json");
                let mut policy: Value =
                    serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
                policy["keys"]
                    .as_array_mut()
                    .unwrap()
                    .iter_mut()
                    .find(|key| key["metadata"]["id"] == f.key)
                    .unwrap()["metadata"]["capabilities"] = json!(["messages.list"]);
                std::fs::write(path, serde_json::to_vec(&policy).unwrap()).unwrap();
            }
            "disconnected" => {
                assert_eq!(
                    response(
                        router.clone(),
                        f.req(
                            &f.owner(),
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
                drop(service);
                let mut approvals =
                    colecto_host::approvals::Approvals::open(f.dir.path(), now()).unwrap();
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
                let router = f.service("127.0.0.1:4173").router();
                assert_eq!(
                    response(
                        router,
                        f.req(
                            &f.owner(),
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
                    &f.owner(),
                    "POST",
                    &format!("/send-requests/{id}/approve"),
                    json!({})
                )
            )
            .await
            .0,
            403
        );
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(30), listener.accept())
                .await
                .is_err()
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
    let service = f.service(&address.to_string());
    let server = tokio::spawn(axum::serve(listener, service.router()).into_future());
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
    assert!(tools.iter().any(|t| t.name == "messages.send"));
    let send_schema = &tools
        .iter()
        .find(|tool| tool.name == "messages.send")
        .unwrap()
        .input_schema;
    assert!(
        send_schema["required"]
            .as_array()
            .unwrap()
            .contains(&json!("draft"))
    );
    assert_eq!(
        send_schema["properties"]["draft"]["required"],
        json!(["recipient", "subject", "body"])
    );
    assert!(!tools.iter().any(|t| t.name.contains("approve")));
    let arguments = json!({"account":"demo"}).as_object().unwrap().clone();
    let result = http
        .call_tool(CallToolRequestParams::new("messages.list").with_arguments(arguments.clone()))
        .await
        .unwrap();
    assert!(!result.is_error.unwrap_or(false));
    let denied = http
        .call_tool(
            CallToolRequestParams::new("messages.list").with_arguments(
                json!({"account":"not-granted"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    assert_eq!(denied.is_error, Some(true));
    let injected = http
        .call_tool(
            CallToolRequestParams::new("messages.send").with_arguments(
                json!({"account":f.account,"draft":message(),"approved":true})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    assert_eq!(injected.is_error, Some(true));
    let pending = http
        .call_tool(
            CallToolRequestParams::new("messages.send").with_arguments(
                json!({"account":f.account,"draft":message()})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    let pending = pending.structured_content.unwrap();
    assert_eq!(pending["status"], "pending");
    let wrong_account = http
        .call_tool(
            CallToolRequestParams::new("send.status").with_arguments(
                json!({"account":"demo", "id":pending["id"]})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    assert_eq!(wrong_account.structured_content.unwrap()["status"], 404);
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
        let path = f.dir.path().join("api-keys.json");
        let mut policy: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        let expires_at = now() + 1;
        policy["keys"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|key| key["metadata"]["id"] == f.key)
            .unwrap()["metadata"]["expires_at"] = json!(expires_at);
        std::fs::write(path, serde_json::to_vec(&policy).unwrap()).unwrap();
        while now() < expires_at {
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    } else {
        ApiKeys::open(f.dir.path())
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
