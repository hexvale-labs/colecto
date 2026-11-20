//! Named, capability-gated delivery without discovery.
use super::*;
use serde_json::{Value, json};
use std::{cell::Cell, collections::HashMap, time::Duration};
type Pending = (String, tokio::sync::oneshot::Sender<Value>);
pub(crate) type Replies = Arc<Mutex<HashMap<String, Pending>>>;
thread_local! { static DEPTH: Cell<u8> = const { Cell::new(0) }; }
pub(crate) use crate::package::manifest::valid_bus_name as valid_name;
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Input {
    name: String,
    payload: Value,
}
fn validate(input: &Input) -> Result<(), String> {
    if !valid_name(&input.name)
        || serde_json::to_vec(&input.payload)
            .map_err(|e| e.to_string())?
            .len()
            > 32768
    {
        return Err("Invalid bus name or payload exceeds 32 KiB".into());
    }
    Ok(())
}
fn invoke(host: &Host, caller: &str, account: &str, bus: &str, input: &Input) -> Value {
    let depth = DEPTH.with(Cell::get);
    if depth >= 4 {
        return Value::Null;
    }
    let service = {
        let Ok(extensions) = host.extensions.lock() else {
            return Value::Null;
        };
        if !extensions.allows(bus, account, Capability::ExtensionsBus) {
            return Value::Null;
        }
        extensions.service(bus)
    };
    // Without a service part, the extension answers in its client part (or, for bundled
    // extensions not yet on the runtime, its Svelte code), through the event stream.
    let Some((epoch, module)) = service else {
        let Ok(id) = api_keys::random() else {
            return Value::Null;
        };
        let (send, mut receive) = tokio::sync::oneshot::channel();
        if let Ok(mut replies) = host.bus_replies.lock() {
            replies.insert(id.clone(), (bus.into(), send));
        } else {
            return Value::Null;
        }
        let _ = host.events.send((
            "bus.request",
            json!({"id":id,"bus":bus,"from":caller,"name":input.name,"payload":input.payload})
                .to_string(),
        ));
        let result = tokio::runtime::Handle::current().block_on(async {
            tokio::time::timeout(Duration::from_secs(2), &mut receive)
                .await
                .ok()
                .and_then(Result::ok)
                .unwrap_or(Value::Null)
        });
        if let Ok(mut replies) = host.bus_replies.lock() {
            replies.remove(&id);
        }
        return result;
    };
    DEPTH.with(|d| d.set(depth + 1));
    let result = deliver(
        host,
        bus,
        account,
        epoch,
        &module,
        json!({"event":"bus.request","from":caller,"name":input.name,"payload":input.payload}),
    );
    DEPTH.with(|d| d.set(depth));
    result.unwrap_or(Value::Null)["bus"]["response"].clone()
}
fn deliver(
    host: &Host,
    id: &str,
    account: &str,
    epoch: u64,
    module: &sandbox::Compiled,
    action: Value,
) -> Result<Value, String> {
    let (host, id, account) = (host.clone(), id.to_owned(), account.to_owned());
    sandbox::invoke(
        module,
        &action.to_string(),
        Box::new(move |request| extension_service(&host, &id, &account, epoch, request)),
    )
}
async fn publish(host: Host, bus: String, account: String, input: Input) {
    let topic = format!("{bus}/{}", input.name);
    let (targets, clients) = host
        .extensions
        .lock()
        .map(|e| {
            let subscribers = e.list(&account).into_iter().filter(|s| {
                s.enabled
                    && s.manifest.capabilities.contains(&Capability::ExtensionsBus)
                    && s.manifest
                        .bus
                        .as_ref()
                        .is_some_and(|b| b.subscribe.contains(&topic))
            });
            let mut clients = false;
            let mut targets = Vec::new();
            for s in subscribers {
                if s.manifest.client.is_some() {
                    clients = true;
                }
                if let Some((epoch, module)) = e.service(&s.manifest.id) {
                    targets.push((s.manifest.id, epoch, module));
                }
            }
            (targets, clients)
        })
        .unwrap_or_default();
    // Subscribers with a client part receive it over the event stream.
    if clients {
        let _ = host.events.send((
            "bus.event",
            json!({"bus":bus,"name":input.name,"payload":input.payload}).to_string(),
        ));
    }
    let mut tasks = tokio::task::JoinSet::new();
    for (id, epoch, module) in targets {
        let (host, account, action) = (
            host.clone(),
            account.clone(),
            json!({"event":"bus.event","bus":bus,"name":input.name,"payload":input.payload}),
        );
        tasks.spawn(async move {
            let Ok(permit) = host.execution.clone().acquire_owned().await else {
                return;
            };
            let _ = tokio::task::spawn_blocking(move || {
                let _permit = permit;
                let _ = deliver(&host, &id, &account, epoch, &module, action);
            })
            .await;
        });
    }
    while tasks.join_next().await.is_some() {}
}
/// New mail: every enabled service part with `messages.list` runs once with
/// `{"event":"mail","account","ids"}`, in the background and with today's limits.
const MAIL_IDS_PER_INVOCATION: usize = 10;
pub(crate) fn deliver_mail(host: &Host, account: &str, event: &str) {
    let Ok(event) = serde_json::from_str::<Value>(event) else {
        return;
    };
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        return;
    };
    let targets = host
        .extensions
        .lock()
        .map(|e| e.mail_services())
        .unwrap_or_default();
    // A few ids per invocation: each gets a fresh instance with its own request budget
    // (`sandbox::MAX_REQUESTS`), which a service part reading each message would exhaust.
    let ids = event["ids"].as_array().cloned().unwrap_or_default();
    for (id, epoch, module) in targets {
        let host = host.clone();
        let account = account.to_owned();
        let chunks: Vec<Value> = ids
            .chunks(MAIL_IDS_PER_INVOCATION)
            .map(|c| json!(c))
            .collect();
        runtime.spawn(async move {
            for chunk in chunks {
                let Ok(permit) = host.execution.clone().acquire_owned().await else {
                    return;
                };
                let (host, id, account, module) =
                    (host.clone(), id.clone(), account.clone(), module.clone());
                let action = json!({"event": "mail", "account": account, "ids": chunk});
                let _ = tokio::task::spawn_blocking(move || {
                    let _permit = permit;
                    if let Err(error) = deliver(&host, &id, &account, epoch, &module, action) {
                        tracing::warn!(extension = %id, %error, "A service part failed on new mail");
                    }
                })
                .await;
            }
        });
    }
}
pub(super) fn service(
    host: &Host,
    extension: &str,
    account: &str,
    epoch: u64,
    request: Value,
) -> Result<Value, String> {
    if !host
        .extensions
        .lock()
        .map_err(|_| "Policy unavailable")?
        .allows_generation(extension, account, Capability::ExtensionsBus, epoch)
    {
        return Err("Capability not granted".into());
    }
    let input = Input {
        name: request["name"].as_str().ok_or("Name required")?.into(),
        payload: request["payload"].clone(),
    };
    validate(&input)?;
    if request["operation"] == "bus.emit" {
        tokio::runtime::Handle::current().spawn(publish(
            host.clone(),
            extension.into(),
            account.into(),
            input,
        ));
        Ok(Value::Null)
    } else {
        Ok(invoke(
            host,
            extension,
            account,
            request["bus"].as_str().ok_or("Bus required")?,
            &input,
        ))
    }
}
/// Who is calling: a client part's credential names its extension; bundled Svelte that is not
/// on the runtime yet names itself in `x-colecto-extension` behind the owner bearer.
fn identity(
    host: &Host,
    principal: &Principal,
    headers: &HeaderMap,
) -> Result<(String, String), Failure> {
    let account = grant(host, principal, headers, Capability::ExtensionsBus)?;
    let caller = match principal {
        Principal::Extension(token) => caller_extension(host, token)?.0,
        _ => headers
            .get("x-colecto-extension")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(forbidden)?
            .into(),
    };
    Ok((caller, account))
}
pub(super) async fn request_http(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Path(bus): Path<String>,
    Json(input): Json<Input>,
) -> Result<Json<Value>, Failure> {
    let (caller, account) = identity(&host, &principal, &headers)?;
    validate(&input).map_err(|e| Failure(StatusCode::BAD_REQUEST, e))?;
    let permit = host
        .execution
        .clone()
        .try_acquire_owned()
        .map_err(|_| Failure(StatusCode::TOO_MANY_REQUESTS, "Extensions busy".into()))?;
    let data = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        invoke(&host, &caller, &account, &bus, &input)
    })
    .await
    .unwrap_or(Value::Null);
    Ok(Json(json!({"data":data})))
}
pub(super) async fn emit_http(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Json(input): Json<Input>,
) -> Result<Json<Value>, Failure> {
    let (caller, account) = identity(&host, &principal, &headers)?;
    validate(&input).map_err(|e| Failure(StatusCode::BAD_REQUEST, e))?;
    let task = tokio::spawn(publish(host, caller, account, input));
    let _ = tokio::time::timeout(Duration::from_millis(1500), task).await;
    Ok(Json(json!({})))
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Reply {
    data: Value,
}
pub(super) async fn reply_http(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(reply): Json<Reply>,
) -> Result<Json<Value>, Failure> {
    let (caller, _) = identity(&host, &principal, &headers)?;
    if serde_json::to_vec(&reply.data)
        .map_err(|_| forbidden())?
        .len()
        > 32768
    {
        return Err(Failure(StatusCode::BAD_REQUEST, "Reply too large".into()));
    }
    let mut replies = host.bus_replies.lock().map_err(|_| forbidden())?;
    if replies.get(&id).is_some_and(|(bus, _)| bus == &caller)
        && let Some((_, send)) = replies.remove(&id)
    {
        let _ = send.send(reply.data);
    }
    Ok(Json(json!({})))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn host() -> (Host, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        (crate::test_host(dir.path()), dir)
    }
    fn install(host: &Host, id: &str, output: &str, request: bool) {
        let escaped: String = output.bytes().map(|b| format!("\\{b:02x}")).collect();
        let module = if request {
            format!(
                r#"(module (import "colecto" "request" (func $request (param i32 i32 i32 i32) (result i32))) (import "colecto" "emit" (func $emit (param i32 i32) (result i32))) (memory (export "memory") 2) (data (i32.const 0) "{escaped}") (data (i32.const 1000) "{{\22bus\22:{{\22response\22:") (data (i32.const 1100) "}}}}") (func (export "run") (local $n i32) i32.const 0 i32.const {} i32.const 2000 i32.const 10000 call $request local.set $n i32.const 1019 i32.const 2000 local.get $n memory.copy i32.const 1019 local.get $n i32.add i32.const 1100 i32.const 2 memory.copy i32.const 1000 local.get $n i32.const 21 i32.add call $emit drop))"#,
                output.len()
            )
        } else {
            format!(
                r#"(module (import "colecto" "emit" (func $emit (param i32 i32) (result i32))) (memory (export "memory") 1) (data (i32.const 0) "{escaped}") (func (export "run") i32.const 0 i32.const {} call $emit drop))"#,
                output.len()
            )
        };
        let source = format!(
            "id = {id:?}\nname = 'Test'\ndescription = 'Test'\nauthor = 'Test'\napi_version = 1\nclient = 'client/main.js'\ncapabilities = ['extensions.bus']"
        );
        host.extensions
            .lock()
            .unwrap()
            .install(&extensions::tests::package(
                &source,
                Some(wat::parse_str(module).unwrap()),
            ))
            .unwrap();
    }
    #[tokio::test]
    async fn bundled_reply_is_bound_to_bus_and_first_reply_wins() {
        let (host, _dir) = host();
        let mut events = host.events.subscribe();
        let worker_host = host.clone();
        let task = tokio::task::spawn_blocking(move || {
            invoke(
                &worker_host,
                "composer",
                "demo",
                "reader",
                &Input {
                    name: "v1/get_html".into(),
                    payload: Value::Null,
                },
            )
        });
        let (event, payload) = events.recv().await.unwrap();
        assert_eq!(event, "bus.request");
        let payload: Value = serde_json::from_str(&payload).unwrap();
        let id = payload["id"].as_str().unwrap().to_owned();
        let mut headers = HeaderMap::new();
        headers.insert("x-colecto-extension", "composer".parse().unwrap());
        headers.insert("x-colecto-account", "demo".parse().unwrap());
        let _ = reply_http(
            State(host.clone()),
            axum::Extension(Principal::Owner),
            headers.clone(),
            Path(id.clone()),
            Json(Reply {
                data: json!("wrong bus"),
            }),
        )
        .await
        .ok()
        .unwrap();
        assert!(host.bus_replies.lock().unwrap().contains_key(&id));
        headers.insert("x-colecto-extension", "reader".parse().unwrap());
        for data in ["first", "second"] {
            let _ = reply_http(
                State(host.clone()),
                axum::Extension(Principal::Owner),
                headers.clone(),
                Path(id.clone()),
                Json(Reply { data: json!(data) }),
            )
            .await
            .ok()
            .unwrap();
        }
        assert_eq!(task.await.unwrap(), json!("first"));
        assert!(host.bus_replies.lock().unwrap().is_empty());
    }
    #[test]
    fn missing_roundtrip_depth_and_permission() {
        let (host, _dir) = host();
        let input = Input {
            name: "v1/test".into(),
            payload: json!({}),
        };
        assert_eq!(
            invoke(&host, "composer", "demo", "missing", &input),
            Value::Null
        );
        install(
            &host,
            "answer",
            r#"{"bus":{"response":{"answer":42}}}"#,
            false,
        );
        assert_eq!(
            invoke(&host, "composer", "demo", "answer", &input),
            json!({"answer":42})
        );
        install(
            &host,
            "relay",
            r#"{"operation":"bus.request","bus":"answer","name":"v1/test","payload":{}}"#,
            true,
        );
        assert_eq!(
            invoke(&host, "composer", "demo", "relay", &input),
            json!({"ok":true,"data":{"answer":42}})
        );
        install(
            &host,
            "cycle",
            r#"{"operation":"bus.request","bus":"cycle","name":"v1/test","payload":{}}"#,
            true,
        );
        let cycle = invoke(&host, "composer", "demo", "cycle", &input);
        assert_eq!(cycle["data"]["data"]["data"]["data"], Value::Null);
        DEPTH.with(|d| d.set(4));
        assert_eq!(
            invoke(&host, "composer", "demo", "answer", &input),
            Value::Null
        );
        DEPTH.with(|d| d.set(0));
        let request =
            json!({"operation":"bus.request","bus":"answer","name":"v1/test","payload":null});
        assert!(service(&host, "ai", "demo", 0, request).is_err());
    }
    #[tokio::test]
    async fn publish_delivers_to_both_client_and_service_when_extension_has_both() {
        let (host, _dir) = host();
        let req = r#"{"operation":"storage.set","key":"ran","value":true}"#;
        let escaped: String = req.bytes().map(|b| format!("\\{b:02x}")).collect();
        let module = format!(
            r#"(module (import "colecto" "request" (func $request (param i32 i32 i32 i32) (result i32))) (memory (export "memory") 1) (data (i32.const 0) "{escaped}") (func (export "run") i32.const 0 i32.const {} i32.const 512 i32.const 1024 call $request drop))"#,
            req.len()
        );
        let source = r#"
id = "both"
name = "Both"
description = "Has both client and service"
author = "Test"
api_version = 1
client = "client/main.js"
capabilities = ["extensions.bus"]
[bus]
subscribe = ["inbox/v1/ready"]
"#;
        host.extensions
            .lock()
            .unwrap()
            .install(&extensions::tests::package(
                source,
                Some(wat::parse_str(module).unwrap()),
            ))
            .unwrap();
        let mut events = host.events.subscribe();
        publish(
            host.clone(),
            "inbox".into(),
            "demo".into(),
            Input {
                name: "v1/ready".into(),
                payload: json!({"ok": true}),
            },
        )
        .await;
        // Client part received the event over the event stream.
        let (event, payload) = events
            .try_recv()
            .expect("client event stream received bus.event");
        assert_eq!(event, "bus.event");
        let payload: Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(payload["bus"], "inbox");
        assert_eq!(payload["name"], "v1/ready");
        // Service part also ran and persisted its storage marker.
        assert_eq!(
            host.extensions.lock().unwrap().stored("both", "ran"),
            json!(true)
        );
    }
}
