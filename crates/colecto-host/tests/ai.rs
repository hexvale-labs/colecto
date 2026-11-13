//! AI routes against a local stand-in for the configured services.
use axum::{
    Json,
    body::{Body, to_bytes},
    http::{HeaderMap, Request, StatusCode},
};
use colecto_core::MailStore;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

const ORIGIN: &str = "http://127.0.0.1:4173";
type Seen = Arc<Mutex<Vec<(String, Option<String>, serde_json::Value)>>>;

/// Records every request and answers like an OpenAI-compatible service and the Decisions API.
/// A request for the model "rejected" gets HTTP 401.
async fn services() -> (String, Seen) {
    let seen: Seen = Arc::default();
    let record = |path: &'static str, seen: Seen| {
        move |headers: HeaderMap, Json(body): Json<serde_json::Value>| async move {
            let auth = headers
                .get("authorization")
                .map(|v| v.to_str().unwrap().to_owned());
            seen.lock().unwrap().push((path.into(), auth, body.clone()));
            if body["model"] == "rejected" {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(serde_json::json!({"error":"bad key sk-echo"})),
                );
            }
            if path == "chat" {
                let system = body["messages"][0]["content"].as_str().unwrap_or("");
                let text = if system.contains("Summarize") {
                    "<think>hmm</think>A short summary."
                } else {
                    "Drafted reply."
                };
                (
                    StatusCode::OK,
                    Json(
                        serde_json::json!({"choices":[{"message":{"role":"assistant","content":text}}]}),
                    ),
                )
            } else {
                (
                    StatusCode::OK,
                    Json(
                        serde_json::json!({"model":"typesafe/jev-1.14","answers":{"q0":{"type":"noul","noul":0.5}},"usage":{"input_tokens":1,"output_tokens":1}}),
                    ),
                )
            }
        }
    };
    let app = axum::Router::new()
        .route(
            "/v1/chat/completions",
            axum::routing::post(record("chat", seen.clone())),
        )
        .route(
            "/api/alpha/decisions",
            axum::routing::post(record("decisions", seen.clone())),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{address}"), seen)
}

fn app(dir: &std::path::Path) -> axum::Router {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(dir.join("owner-token"))
        .unwrap()
        .write_all("a".repeat(64).as_bytes())
        .unwrap();
    colecto_host::app(
        MailStore::open(dir).unwrap(),
        "127.0.0.1:4173".into(),
        dir.join("assets"),
    )
}

async fn call(
    router: &axum::Router,
    method: &str,
    path: &str,
    extension: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value, String) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:4173")
        .header("origin", ORIGIN)
        .header("authorization", format!("Bearer {}", "a".repeat(64)))
        .header("x-colecto-account", "demo")
        .header("x-colecto-extension", extension)
        .header("content-type", "application/json")
        .body(if method == "GET" {
            Body::empty()
        } else {
            Body::from(body.to_string())
        })
        .unwrap();
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1_000_000).await.unwrap();
    let text = String::from_utf8_lossy(&bytes).into_owned();
    (
        status,
        serde_json::from_str(&text).unwrap_or_default(),
        text,
    )
}

fn settings(url: &str, model: &str) -> serde_json::Value {
    serde_json::json!({
        "enabled": true,
        "text": {"base_url": format!("{url}/v1/"), "model": model, "max_tokens": 300, "effort": "low"},
        "decision": {"base_url": url, "model": "typesafe/jev-1.14", "threshold": 0.7},
        "autolabel": false,
        "labels": {},
    })
}

#[tokio::test]
async fn ai_is_off_by_default_and_owner_only() {
    let dir = tempfile::tempdir().unwrap();
    let router = app(dir.path());
    let (status, body, _) = call(
        &router,
        "GET",
        "/api/v1/ai/settings",
        "host",
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["settings"]["enabled"], false);
    assert_eq!(body["settings"]["decision"]["model"], "typesafe/jev-1.14");
    assert_eq!(
        body["keys"],
        serde_json::json!({"text": false, "decision": false})
    );
    // No request is made while AI is off.
    let (status, _, _) = call(
        &router,
        "POST",
        "/api/v1/ai/summaries/welcome",
        "ai",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    // Settings are Host controls; other bundled identities cannot reach them or the AI actions.
    for (method, path, extension) in [
        ("GET", "/api/v1/ai/settings", "ai"),
        ("GET", "/api/v1/ai/settings", "reader"),
        ("POST", "/api/v1/ai/keys", "inbox"),
        ("POST", "/api/v1/ai/summaries/welcome", "reader"),
        ("POST", "/api/v1/ai/reply", "composer"),
    ] {
        let (status, _, _) = call(&router, method, path, extension, serde_json::json!({})).await;
        assert_eq!(
            status,
            StatusCode::FORBIDDEN,
            "{method} {path} as {extension}"
        );
    }
}

#[tokio::test]
async fn summaries_and_replies_reach_the_configured_service_and_keys_never_come_back() {
    let (url, seen) = services().await;
    let dir = tempfile::tempdir().unwrap();
    let router = app(dir.path());
    // The vault needs an unlocked key to seal secrets; a master password provides one here.
    let (status, _, _) = call(
        &router,
        "POST",
        "/api/v1/security",
        "host",
        serde_json::json!({"action":"protect","password":"a strong master password"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = call(
        &router,
        "POST",
        "/api/v1/ai/settings",
        "host",
        settings(&url, "chat-model"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    for (kind, key) in [
        ("text", "sk-text-secret"),
        ("decision", "sk-decision-secret"),
    ] {
        let (status, body, text) = call(
            &router,
            "POST",
            "/api/v1/ai/keys",
            "host",
            serde_json::json!({"kind":kind,"key":key}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["keys"][kind], true);
        assert!(!text.contains(key));
    }
    let (_, _, text) = call(
        &router,
        "GET",
        "/api/v1/ai/settings",
        "host",
        serde_json::Value::Null,
    )
    .await;
    assert!(!text.contains("sk-text-secret") && !text.contains("sk-decision-secret"));

    // Summaries are generated once, kept, and can be deleted.
    let (status, body, _) = call(
        &router,
        "POST",
        "/api/v1/ai/summaries/weekend",
        "ai",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["summary"], "A short summary.");
    let (_, body, _) = call(
        &router,
        "GET",
        "/api/v1/ai/summaries/weekend",
        "ai",
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(body["summary"], "A short summary.");
    {
        let seen = seen.lock().unwrap();
        let (path, auth, request) = &seen[0];
        assert_eq!(path, "chat");
        assert_eq!(auth.as_deref(), Some("Bearer sk-text-secret"));
        assert_eq!(request["model"], "chat-model");
        assert_eq!(request["max_tokens"], 300);
        assert_eq!(request["reasoning_effort"], "low");
        let user = request["messages"][1]["content"].as_str().unwrap();
        assert!(
            user.contains("Subject: A small plan for the weekend")
                && user.contains("little bookshop")
        );
    }
    let (_, body, _) = call(
        &router,
        "DELETE",
        "/api/v1/ai/summaries/weekend",
        "ai",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(body["summary"], serde_json::Value::Null);
    let (_, body, _) = call(
        &router,
        "GET",
        "/api/v1/ai/summaries/weekend",
        "ai",
        serde_json::Value::Null,
    )
    .await;
    assert_eq!(body["summary"], serde_json::Value::Null);

    // Replies carry the original, the owner's notes and the tone.
    let (status, body, _) = call(&router, "POST", "/api/v1/ai/reply", "ai", serde_json::json!({"message_id":"weekend","input":"yeah wtf i do not agree to this","tone":"roast"})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["text"], "Drafted reply.");
    {
        let seen = seen.lock().unwrap();
        let request = &seen.last().unwrap().2;
        let system = request["messages"][0]["content"].as_str().unwrap();
        let user = request["messages"][1]["content"].as_str().unwrap();
        assert!(system.contains("Tone: roast") && system.contains("self-deprecating"));
        assert!(
            user.contains("little bookshop") && user.contains("yeah wtf i do not agree to this")
        );
    }
    let (status, _, _) = call(
        &router,
        "POST",
        "/api/v1/ai/reply",
        "ai",
        serde_json::json!({"message_id":"weekend","input":"x","tone":"evil"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // The decision model test uses its own key and the Decisions API shape.
    let (status, _, _) = call(
        &router,
        "POST",
        "/api/v1/ai/test",
        "host",
        serde_json::json!({"kind":"decision"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    {
        let seen = seen.lock().unwrap();
        let (path, auth, request) = seen.last().unwrap();
        assert_eq!(path, "decisions");
        assert_eq!(auth.as_deref(), Some("Bearer sk-decision-secret"));
        assert_eq!(request["questions"]["q0"]["type"], "noul");
        assert!(request["state"].is_array());
    }

    // A refused key becomes a plain message without the service's own words.
    call(
        &router,
        "POST",
        "/api/v1/ai/settings",
        "host",
        settings(&url, "rejected"),
    )
    .await;
    let (status, body, text) = call(
        &router,
        "POST",
        "/api/v1/ai/summaries/weekend",
        "ai",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(body["error"], "The AI service rejected the API key.");
    assert!(!text.contains("sk-echo"));

    // Removing a key stops it being sent.
    call(
        &router,
        "POST",
        "/api/v1/ai/settings",
        "host",
        settings(&url, "chat-model"),
    )
    .await;
    let (_, body, _) = call(
        &router,
        "POST",
        "/api/v1/ai/keys",
        "host",
        serde_json::json!({"kind":"text","key":null}),
    )
    .await;
    assert_eq!(body["keys"]["text"], false);
    call(
        &router,
        "POST",
        "/api/v1/ai/test",
        "host",
        serde_json::json!({"kind":"text"}),
    )
    .await;
    assert_eq!(seen.lock().unwrap().last().unwrap().1, None);
}

#[tokio::test]
async fn label_rules_are_validated_and_previewed_against_the_saved_inbox() {
    let dir = tempfile::tempdir().unwrap();
    let router = app(dir.path());
    let mut input = settings("http://127.0.0.1:9", "m");
    input["labels"] = serde_json::json!({"demo": {"Work": {"enabled": true, "model": false, "description": "", "rules": [{"field":"any","pattern":"(unclosed"}]}}});
    let (status, body, _) = call(&router, "POST", "/api/v1/ai/settings", "host", input).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(body["error"].as_str().unwrap().contains("Work"));
    let (status, body, _) = call(
        &router,
        "POST",
        "/api/v1/ai/labels/preview",
        "host",
        serde_json::json!({"rule": {"enabled": true, "model": false, "description": "", "rules": [
            {"field":"sender","pattern":"example\\.test$"}, {"field":"subject","pattern":"[oops"}
        ]}}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["errors"][0].is_null() && body["errors"][1].is_string());
    assert_eq!(body["checked"], 4);
    assert_eq!(body["count"], 3);
}
