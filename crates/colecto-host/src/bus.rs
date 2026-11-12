//! Named, capability-gated delivery without discovery.
use super::*;
use serde_json::{Value, json};
use std::{cell::Cell, collections::HashMap, time::Duration};
type Pending = (String, tokio::sync::oneshot::Sender<Value>);
pub(crate) type Replies = Arc<Mutex<HashMap<String, Pending>>>;
thread_local! { static DEPTH: Cell<u8> = const { Cell::new(0) }; }
pub(crate) fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"-_/".contains(&b))
}
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
    let (module, bundled) = {
        let Ok(extensions) = host.extensions.lock() else {
            return Value::Null;
        };
        if !extensions.allows(bus, account, Capability::ExtensionsBus) {
            return Value::Null;
        }
        (extensions.module(bus), extensions.bundled_identity(bus))
    };
    if bundled {
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
    }
    let Some((epoch, module)) = module else {
        return Value::Null;
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
    sandbox::run_output(
        module,
        &action.to_string(),
        Box::new(move |request| extension_service(&host, &id, &account, epoch, request)),
        None,
    )
}
async fn publish(host: Host, bus: String, account: String, input: Input) {
    let targets =
        host.extensions
            .lock()
            .map(|e| {
                e.list(&account)
                    .into_iter()
                    .filter(|s| {
                        s.enabled
                            && s.manifest.capabilities.contains(&Capability::ExtensionsBus)
                            && s.manifest.bus.as_ref().is_some_and(|b| {
                                b.subscribe.contains(&format!("{bus}/{}", input.name))
                            })
                    })
                    .filter_map(|s| {
                        e.module(&s.manifest.id)
                            .map(|(epoch, module)| (s.manifest.id, epoch, module))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
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
        .allows_guest(extension, account, Capability::ExtensionsBus, epoch)
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
fn identity(host: &Host, headers: &HeaderMap) -> Result<(String, String), Failure> {
    let account = grant(host, &Principal::Owner, headers, Capability::ExtensionsBus)?;
    Ok((
        headers
            .get("x-colecto-extension")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(forbidden)?
            .into(),
        account,
    ))
}
pub(super) async fn request_http(
    State(host): State<Host>,
    headers: HeaderMap,
    Path(bus): Path<String>,
    Json(input): Json<Input>,
) -> Result<Json<Value>, Failure> {
    let (caller, account) = identity(&host, &headers)?;
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
    headers: HeaderMap,
    Json(input): Json<Input>,
) -> Result<Json<Value>, Failure> {
    let (caller, account) = identity(&host, &headers)?;
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
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(reply): Json<Reply>,
) -> Result<Json<Value>, Failure> {
    let (caller, _) = identity(&host, &headers)?;
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
        use colecto_core::{Accounts, MailStore};
        let dir = tempfile::tempdir().unwrap();
        let accounts = Accounts::open(
            MailStore::open(dir.path()).unwrap(),
            Arc::new(UnavailableSecrets),
        )
        .unwrap();
        let db = accounts.database();
        (
            Host {
                events: tokio::sync::broadcast::channel(16).0,
                bus_replies: Default::default(),
                keys: Arc::new(Mutex::new(ApiKeys::open(db.clone()).unwrap())),
                preferences: Arc::new(Mutex::new(
                    preferences::PreferenceStore::open(db.clone()).unwrap(),
                )),
                approvals: Arc::new(Mutex::new(
                    approvals::Approvals::open(db, api_keys::now()).unwrap(),
                )),
                status_polls: Arc::default(),
                accounts: Arc::new(Mutex::new(accounts)),
                extensions: Arc::new(Mutex::new(extensions::Extensions::bundled())),
                authority: "localhost".into(),
                origin: "http://localhost".into(),
                execution: Arc::new(tokio::sync::Semaphore::new(2)),
            },
            dir,
        )
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
            "id = {id:?}\nname = 'Test'\ndescription = 'Test'\nauthor = 'Test'\napi_version = 1\nslot = 'panel'\ncapabilities = ['extensions.bus']"
        );
        host.extensions
            .lock()
            .unwrap()
            .install(&source, wat::parse_str(module).unwrap())
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
        assert!(service(&host, "mail-shell", "demo", 0, request).is_err());
    }
}
