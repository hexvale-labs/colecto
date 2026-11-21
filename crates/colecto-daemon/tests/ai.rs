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
                        serde_json::json!({"model":"typesafe/jev-1.14","answers":{"q0":{"type":"noul","noul":0.91}},"usage":{"input_tokens":1,"output_tokens":1}}),
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
    colecto_daemon::app(
        MailStore::open(dir).unwrap(),
        "127.0.0.1:4173".into(),
        "a".repeat(64),
        dir.join("assets"),
    )
}

async fn call(
    router: &axum::Router,
    method: &str,
    path: &str,
    body: serde_json::Value,
) -> (StatusCode, serde_json::Value, String) {
    let request = Request::builder()
        .method(method)
        .uri(path)
        .header("host", "127.0.0.1:4173")
        .header("origin", ORIGIN)
        .header("authorization", format!("Bearer {}", "a".repeat(64)))
        .header("x-colecto-account", "demo")
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
async fn ai_is_off_by_default() {
    let dir = tempfile::tempdir().unwrap();
    let router = app(dir.path());
    let (status, body, _) = call(
        &router,
        "GET",
        "/api/v1/ai/settings",
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
    let (status, _, _) = call(
        &router,
        "POST",
        "/api/v1/ai/summaries/welcome",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn summaries_replies_reclassify_and_keys_never_come_back() {
    let (url, seen) = services().await;
    let dir = tempfile::tempdir().unwrap();
    let router = app(dir.path());
    let (status, _, _) = call(
        &router,
        "POST",
        "/api/v1/security",
        serde_json::json!({"action":"protect","password":"a strong master password"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = call(
        &router,
        "POST",
        "/api/v1/ai/settings",
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
        serde_json::Value::Null,
    )
    .await;
    assert!(!text.contains("sk-text-secret") && !text.contains("sk-decision-secret"));

    // Summaries persist in colecto.db and can be deleted.
    let (status, body, _) = call(
        &router,
        "POST",
        "/api/v1/ai/summaries/weekend",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["summary"], "A short summary.");
    let (_, body, _) = call(
        &router,
        "GET",
        "/api/v1/ai/summaries/weekend",
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
    }
    let (_, body, _) = call(
        &router,
        "DELETE",
        "/api/v1/ai/summaries/weekend",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(body["summary"], serde_json::Value::Null);

    // Replies carry original, notes, and tone.
    let (status, body, _) = call(
        &router,
        "POST",
        "/api/v1/ai/reply",
        serde_json::json!({"message_id":"weekend","input":"not this weekend","tone":"roast"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["text"], "Drafted reply.");

    // Reclassify via HTTP applies rules and decision model natively.
    call(
        &router,
        "POST",
        "/api/v1/labels",
        serde_json::json!({"name": "Friends"}),
    )
    .await;
    let mut cfg = settings(&url, "chat-model");
    cfg["labels"] = serde_json::json!({
        "demo": {
            "Friends": {"enabled": true, "model": true, "description": "Friends", "rules": []}
        }
    });
    assert_eq!(
        call(&router, "POST", "/api/v1/ai/settings", cfg).await.0,
        StatusCode::OK
    );
    let (status, body, _) = call(
        &router,
        "POST",
        "/api/v1/ai/reclassify",
        serde_json::json!({"ids": ["weekend"]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["checked"], 1);
    assert_eq!(body["changed"], 1);

    // Rejected key becomes a redacted 502 error.
    call(
        &router,
        "POST",
        "/api/v1/ai/settings",
        settings(&url, "rejected"),
    )
    .await;
    let (status, body, text) = call(
        &router,
        "POST",
        "/api/v1/ai/summaries/weekend",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert_eq!(body["error"], "The AI service rejected the API key.");
    assert!(!text.contains("sk-echo"));
}

#[tokio::test]
async fn changing_base_url_clears_saved_key_and_requires_unlocked_vault() {
    let (url1, seen1) = services().await;
    let (url2, seen2) = services().await;
    let dir = tempfile::tempdir().unwrap();
    let router = app(dir.path());
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/v1/security",
            serde_json::json!({"action":"protect","password":"a strong master password"}),
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/v1/ai/settings",
            settings(&url1, "chat-model")
        )
        .await
        .0,
        StatusCode::OK
    );
    for (kind, key) in [
        ("text", "sk-text-secret"),
        ("decision", "sk-decision-secret"),
    ] {
        let (status, body, _) = call(
            &router,
            "POST",
            "/api/v1/ai/keys",
            serde_json::json!({"kind":kind,"key":key}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["keys"][kind], true);
    }

    // Moving text.base_url clears only text key.
    let mut moved = settings(&url1, "chat-model");
    moved["text"]["base_url"] = serde_json::json!(format!("{url2}/v1"));
    let (status, body, _) = call(&router, "POST", "/api/v1/ai/settings", moved).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        body["keys"],
        serde_json::json!({"text": false, "decision": true})
    );
    assert_eq!(
        call(
            &router,
            "POST",
            "/api/v1/ai/test",
            serde_json::json!({"kind":"text"}),
        )
        .await
        .0,
        StatusCode::OK
    );
    assert!(seen1.lock().unwrap().is_empty());
    assert_eq!(seen2.lock().unwrap().last().unwrap().1, None);

    // Locked vault refuses base_url change with 409.
    call(
        &router,
        "POST",
        "/api/v1/ai/keys",
        serde_json::json!({"kind":"text","key":"sk-locked"}),
    )
    .await;
    call(
        &router,
        "POST",
        "/api/v1/security",
        serde_json::json!({"action":"lock"}),
    )
    .await;
    let (status, _, _) = call(
        &router,
        "POST",
        "/api/v1/ai/settings",
        settings(&url1, "chat-model"),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}
