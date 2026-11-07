//! Trusted HTTP adapter for bundled UI and isolated Wasm extensions.
mod extensions;
pub mod notifications;
pub mod runtime;
mod service;
pub use service::{AccessPolicy, MailService};
mod sandbox;

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, Request, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::Next,
    response::{IntoResponse, Response},
    routing::get,
};
use colecto_core::{Accounts, Error, MailStore, SecretStore};
use colecto_extension_api::{
    AccountInfo, AccountInput, ApiError, Capability, Draft, DraftInput, ExtensionInstallInput,
    ExtensionPolicyInput, ExtensionRunInput, ExtensionState, ExtensionView, Message, SecurityInfo,
    SecurityInput,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tower_http::services::ServeDir;

#[derive(Clone)]
struct Host {
    accounts: Arc<Mutex<Accounts>>,
    extensions: Arc<Mutex<extensions::Extensions>>,
    authority: String,
    origin: String,
    execution: Arc<tokio::sync::Semaphore>,
}

struct Failure(StatusCode, String);
impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        (self.0, Json(ApiError { error: self.1 })).into_response()
    }
}
impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        match error {
            Error::Transport(_) => Self(StatusCode::BAD_GATEWAY, error.to_string()),
            Error::Missing => Self(StatusCode::NOT_FOUND, error.to_string()),
            Error::Invalid(_) => Self(StatusCode::BAD_REQUEST, error.to_string()),
            Error::Storage(_) => {
                eprintln!("Local mail storage error: {error}");
                Self(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Could not access local mail storage. Your unsaved changes are still in the editor.".into(),
                )
            }
        }
    }
}
fn forbidden() -> Failure {
    Failure(
        StatusCode::FORBIDDEN,
        "This request is not granted access".into(),
    )
}

/// Construct the Host with an explicit loopback authority and trusted static build.
pub fn app(store: MailStore, authority: String, assets: PathBuf) -> Router {
    let accounts = Accounts::open(store, Arc::new(UnavailableSecrets)).expect("open accounts");
    app_with_accounts(accounts, authority, assets)
}

pub fn app_with_accounts(accounts: Accounts, authority: String, assets: PathBuf) -> Router {
    MailService::new(accounts, AccessPolicy::local(authority), assets)
        .expect("Cannot initialize mail service")
        .router()
}

fn build_app(host: Host, assets: PathBuf) -> Router {
    let api = Router::new()
        .route("/extensions", get(list_extensions).post(install_extension))
        .route(
            "/extensions/{id}",
            axum::routing::post(update_extension).delete(remove_extension),
        )
        .route("/extensions/{id}/run", axum::routing::post(run_extension))
        .route("/accounts", get(list_accounts).post(upsert_account))
        .route(
            "/accounts/{id}/disconnect",
            axum::routing::post(disconnect_account),
        )
        .route("/security", get(security).post(security_action))
        .route("/account", get(account))
        .route("/send", axum::routing::post(send))
        .route("/labels", get(labels).post(update_label))
        .route("/messages", get(messages))
        .route("/messages/{id}", get(message))
        .route("/drafts", get(drafts).post(save_draft))
        .route("/drafts/{id}", get(draft))
        .fallback(|| async { Failure(StatusCode::NOT_FOUND, "API route not found".into()) });
    Router::new()
        .nest("/api/v1", api)
        .fallback_service(ServeDir::new(assets))
        .layer(DefaultBodyLimit::max(512_000))
        .with_state(host)
}

async fn boundary(State(host): State<Host>, request: Request, next: Next) -> Response {
    let headers = request.headers();
    let authority_ok =
        headers.get(header::HOST).and_then(|v| v.to_str().ok()) == Some(&host.authority);
    let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());
    let mutation = !matches!(
        *request.method(),
        axum::http::Method::GET | axum::http::Method::HEAD
    );
    let origin_ok = match origin {
        Some(value) => value == host.origin,
        None => !mutation,
    };
    let mut response = if !authority_ok || !origin_ok {
        forbidden().into_response()
    } else {
        next.run(request).await
    };
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(
            "default-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'",
        ),
    );
    headers.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    headers.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

// Headers route trusted bundled callers only. Wasm identities are bound by the
// Host invocation and can never use this direct HTTP service path.
fn grant(host: &Host, headers: &HeaderMap, operation: Capability) -> Result<String, Failure> {
    let account = headers
        .get("x-colecto-account")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(forbidden)?;
    let extension = headers
        .get("x-colecto-extension")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(forbidden)?;
    let extensions = host.extensions.lock().map_err(|_| forbidden())?;
    if extensions.bundled_identity(extension) && extensions.allows(extension, account, operation) {
        Ok(account.to_owned())
    } else {
        Err(forbidden())
    }
}

async fn list_extensions(
    State(host): State<Host>,
    headers: HeaderMap,
) -> Result<Json<Vec<ExtensionState>>, Failure> {
    host_grant(&headers)?;
    let account = extension_account(&host, &headers)?;
    Ok(Json(
        host.extensions
            .lock()
            .map_err(|_| forbidden())?
            .list(&account),
    ))
}
fn extension_account(host: &Host, headers: &HeaderMap) -> Result<String, Failure> {
    let account = headers
        .get("x-colecto-account")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(forbidden)?;
    if !host
        .accounts
        .lock()
        .map_err(|_| forbidden())?
        .list()
        .iter()
        .any(|a| a.id == account)
    {
        return Err(forbidden());
    }
    Ok(account.into())
}
async fn update_extension(
    State(host): State<Host>,
    headers: HeaderMap,
    Path(id): Path<String>,
    input: Result<Json<ExtensionPolicyInput>, JsonRejection>,
) -> Result<Json<bool>, Failure> {
    host_grant(&headers)?;
    let account = extension_account(&host, &headers)?;
    let Json(input) =
        input.map_err(|_| Failure(StatusCode::BAD_REQUEST, "Invalid extension policy".into()))?;
    host.extensions
        .lock()
        .map_err(|_| forbidden())?
        .update(&id, &account, input.enabled, input.grants)
        .map_err(|error| {
            if error == "Capabilities not declared by extension" { forbidden() }
            else { Failure(StatusCode::INTERNAL_SERVER_ERROR, "Could not confirm extension policy persistence. Reload permissions before retrying.".into()) }
        })?;
    Ok(Json(true))
}

async fn install_extension(
    State(host): State<Host>,
    headers: HeaderMap,
    input: Result<Json<ExtensionInstallInput>, JsonRejection>,
) -> Result<Json<bool>, Failure> {
    host_grant(&headers)?;
    let Json(input) =
        input.map_err(|_| Failure(StatusCode::BAD_REQUEST, "Invalid extension package".into()))?;
    tokio::task::spawn_blocking(move || {
        host.extensions
            .lock()
            .map_err(|_| forbidden())?
            .install(&input.manifest, input.module)
            .map_err(|e| Failure(StatusCode::BAD_REQUEST, e))
    })
    .await
    .map_err(|_| {
        Failure(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Extension installation failed".into(),
        )
    })??;
    Ok(Json(true))
}
async fn remove_extension(
    State(host): State<Host>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<bool>, Failure> {
    host_grant(&headers)?;
    host.extensions
        .lock()
        .map_err(|_| forbidden())?
        .remove(&id)
        .map_err(|e| Failure(StatusCode::BAD_REQUEST, e))?;
    Ok(Json(true))
}
async fn run_extension(
    State(host): State<Host>,
    headers: HeaderMap,
    Path(id): Path<String>,
    input: Result<Json<ExtensionRunInput>, JsonRejection>,
) -> Result<Json<ExtensionView>, Failure> {
    host_grant(&headers)?;
    let account = extension_account(&host, &headers)?;
    let Json(input) =
        input.map_err(|_| Failure(StatusCode::BAD_REQUEST, "Invalid extension action".into()))?;
    let (epoch, module) = host
        .extensions
        .lock()
        .map_err(|_| forbidden())?
        .module(&id)
        .ok_or_else(forbidden)?;
    let permit = host.execution.clone().try_acquire_owned().map_err(|_| {
        Failure(
            StatusCode::TOO_MANY_REQUESTS,
            "Extensions busy; try again".into(),
        )
    })?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        sandbox::run(
            &module,
            &input.action,
            Box::new(move |request| extension_service(&host, &id, &account, epoch, request)),
        )
        .map(Json)
        .map_err(|_| {
            Failure(
                StatusCode::BAD_REQUEST,
                "Extension failed or exceeded its limits".into(),
            )
        })
    })
    .await
    .map_err(|_| {
        Failure(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Extension runtime unavailable".into(),
        )
    })?
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ServiceRequest {
    operation: Capability,
    id: Option<String>,
    draft: Option<DraftInput>,
    label: Option<colecto_extension_api::LabelInput>,
}
fn extension_service(
    host: &Host,
    extension: &str,
    account: &str,
    epoch: u64,
    request: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let request: ServiceRequest =
        serde_json::from_value(request).map_err(|_| "Invalid service request")?;
    // Identity and account come from the Host invocation, never from guest input.
    let extensions = host
        .extensions
        .lock()
        .map_err(|_| "Extension policy unavailable")?;
    if !extensions.allows_guest(extension, account, request.operation, epoch) {
        return Err("Capability not granted".into());
    }
    // Already authorized operations may finish after a later revocation.
    drop(extensions);
    let handle = host
        .accounts
        .lock()
        .map_err(|_| "Account service unavailable")?
        .store(account)
        .map_err(|_| "Account unavailable")?;
    let mut store = handle.try_lock().map_err(|_| "Account busy; try again")?;
    use Capability::*;
    let result = match request.operation {
        AccountRead => serde_json::to_value(store.connected()),
        MessagesList => serde_json::to_value(store.messages()),
        MessagesRead => serde_json::to_value(
            store
                .message(request.id.as_deref().ok_or("Message ID required")?)
                .ok_or("Message not found")?,
        ),
        DraftsList => serde_json::to_value(store.drafts()),
        DraftsRead => serde_json::to_value(
            store
                .draft(request.id.as_deref().ok_or("Draft ID required")?)
                .ok_or("Draft not found")?,
        ),
        DraftsWrite => serde_json::to_value(
            store
                .save_draft(request.draft.ok_or("Draft required")?)
                .map_err(|e| e.to_string())?,
        ),
        MessagesLabel => {
            let input = request.label.ok_or("Label input required")?;
            serde_json::to_value(
                store
                    .update_label(&input.name, input.message_id.as_deref(), input.remove)
                    .map_err(|e| e.to_string())?,
            )
        }
        MessagesSend => {
            store
                .send(request.draft.ok_or("Draft required")?)
                .map_err(|_| "Mail delivery failed")?;
            serde_json::to_value(true)
        }
    };
    result.map_err(|_| "Cannot serialize service response".into())
}

async fn with_store<T: Send + 'static>(
    host: Host,
    account: String,
    action: impl FnOnce(&mut MailStore) -> Result<T, Error> + Send + 'static,
) -> Result<T, Failure> {
    tokio::task::spawn_blocking(move || {
        let handle = host
            .accounts
            .lock()
            .map_err(|_| forbidden())?
            .store(&account)
            .map_err(|_| forbidden())?;
        let mut store = handle.lock().map_err(|_| {
            Failure(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Mail store unavailable".into(),
            )
        })?;
        action(&mut store).map_err(Failure::from)
    })
    .await
    .map_err(|_| {
        Failure(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Mail service unavailable".into(),
        )
    })?
}
#[derive(serde::Deserialize, Default)]
struct MessageQuery {
    #[serde(default = "inbox_folder")]
    folder: String,
    #[serde(default)]
    q: String,
}
fn inbox_folder() -> String {
    "inbox".into()
}
async fn labels(
    State(host): State<Host>,
    headers: HeaderMap,
) -> Result<Json<std::collections::BTreeMap<String, Vec<String>>>, Failure> {
    let account = grant(&host, &headers, Capability::MessagesList)?;
    with_store(host, account, |s| s.labels()).await.map(Json)
}
async fn update_label(
    State(host): State<Host>,
    headers: HeaderMap,
    input: Result<Json<colecto_extension_api::LabelInput>, JsonRejection>,
) -> Result<Json<std::collections::BTreeMap<String, Vec<String>>>, Failure> {
    let account = grant(&host, &headers, Capability::MessagesLabel)?;
    let Json(input) = input.map_err(|error| Failure(StatusCode::BAD_REQUEST, error.body_text()))?;
    with_store(host, account, move |s| {
        s.update_label(&input.name, input.message_id.as_deref(), input.remove)
    })
    .await
    .map(Json)
}
async fn messages(
    State(host): State<Host>,
    headers: HeaderMap,
    Query(query): Query<MessageQuery>,
) -> Result<Json<Vec<Message>>, Failure> {
    let account = grant(&host, &headers, Capability::MessagesList)?;
    with_store(host, account, move |s| s.query(&query.folder, &query.q))
        .await
        .map(Json)
}
async fn message(
    State(host): State<Host>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Message>, Failure> {
    let account = grant(&host, &headers, Capability::MessagesRead)?;
    with_store(host, account, move |s| {
        s.message(&id).cloned().ok_or(Error::Missing)
    })
    .await
    .map(Json)
}
async fn drafts(State(host): State<Host>, headers: HeaderMap) -> Result<Json<Vec<Draft>>, Failure> {
    let account = grant(&host, &headers, Capability::DraftsList)?;
    with_store(host, account, |s| Ok(s.drafts().to_vec()))
        .await
        .map(Json)
}
async fn draft(
    State(host): State<Host>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Draft>, Failure> {
    let account = grant(&host, &headers, Capability::DraftsRead)?;
    with_store(host, account, move |s| {
        s.draft(&id).cloned().ok_or(Error::Missing)
    })
    .await
    .map(Json)
}
async fn save_draft(
    State(host): State<Host>,
    headers: HeaderMap,
    input: Result<Json<DraftInput>, JsonRejection>,
) -> Result<Json<Draft>, Failure> {
    let account = grant(&host, &headers, Capability::DraftsWrite)?;
    let Json(input) = input.map_err(|error| Failure(StatusCode::BAD_REQUEST, error.body_text()))?;
    with_store(host, account, move |s| s.save_draft(input))
        .await
        .map(Json)
}

async fn account(State(host): State<Host>, headers: HeaderMap) -> Result<Json<bool>, Failure> {
    let account = grant(&host, &headers, Capability::AccountRead)?;
    with_store(host, account, |s| Ok(s.connected()))
        .await
        .map(Json)
}
async fn send(
    State(host): State<Host>,
    headers: HeaderMap,
    input: Result<Json<DraftInput>, JsonRejection>,
) -> Result<Json<bool>, Failure> {
    let account = grant(&host, &headers, Capability::MessagesSend)?;
    let Json(input) = input.map_err(|error| Failure(StatusCode::BAD_REQUEST, error.body_text()))?;
    with_store(host, account, move |s| {
        s.send(input)?;
        Ok(true)
    })
    .await
    .map(Json)
}

// The public compatibility constructor deliberately has no persistence backend.
// Production supplies the platform adapter through app_with_accounts.
struct UnavailableSecrets;
impl SecretStore for UnavailableSecrets {
    fn get(&self, _: &str) -> Result<String, Error> {
        Err(Error::Invalid("System credential store unavailable".into()))
    }
    fn set(&self, _: &str, _: &str) -> Result<(), Error> {
        Err(Error::Invalid("System credential store unavailable".into()))
    }
    fn delete(&self, _: &str) -> Result<(), Error> {
        Err(Error::Invalid("System credential store unavailable".into()))
    }
}
fn host_grant(headers: &HeaderMap) -> Result<(), Failure> {
    if headers
        .get("x-colecto-extension")
        .and_then(|v| v.to_str().ok())
        != Some("host")
    {
        return Err(forbidden());
    }
    Ok(())
}
async fn with_accounts<T: Send + 'static>(
    host: Host,
    action: impl FnOnce(&mut Accounts) -> Result<T, Error> + Send + 'static,
) -> Result<T, Failure> {
    tokio::task::spawn_blocking(move || {
        let mut accounts = host.accounts.lock().map_err(|_| forbidden())?;
        action(&mut accounts).map_err(Failure::from)
    })
    .await
    .map_err(|_| {
        Failure(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Account service unavailable".into(),
        )
    })?
}
async fn list_accounts(
    State(host): State<Host>,
    headers: HeaderMap,
) -> Result<Json<Vec<AccountInfo>>, Failure> {
    host_grant(&headers)?;
    with_accounts(host, |a| Ok(a.list())).await.map(Json)
}
async fn upsert_account(
    State(host): State<Host>,
    headers: HeaderMap,
    input: Result<Json<AccountInput>, JsonRejection>,
) -> Result<Json<AccountInfo>, Failure> {
    host_grant(&headers)?;
    let Json(input) =
        input.map_err(|_| Failure(StatusCode::BAD_REQUEST, "Invalid account settings".into()))?;
    with_accounts(host, move |a| a.upsert(input))
        .await
        .map(Json)
}
async fn disconnect_account(
    State(host): State<Host>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<bool>, Failure> {
    host_grant(&headers)?;
    with_accounts(host, move |a| {
        a.disconnect(&id)?;
        Ok(true)
    })
    .await
    .map(Json)
}
async fn security(
    State(host): State<Host>,
    headers: HeaderMap,
) -> Result<Json<SecurityInfo>, Failure> {
    host_grant(&headers)?;
    with_accounts(host, |a| Ok(a.security())).await.map(Json)
}
async fn security_action(
    State(host): State<Host>,
    headers: HeaderMap,
    input: Result<Json<SecurityInput>, JsonRejection>,
) -> Result<Json<SecurityInfo>, Failure> {
    host_grant(&headers)?;
    let Json(mut input) =
        input.map_err(|_| Failure(StatusCode::BAD_REQUEST, "Invalid security request".into()))?;
    with_accounts(host, move |a| {
        let password = zeroize::Zeroizing::new(std::mem::take(&mut input.password));
        let result = match input.action.as_str() {
            "protect" => a.protect(&password),
            "unprotect" => a.unprotect(),
            "unlock" => a.unlock(&password),
            "lock" => a.lock(),
            "reset" => a.reset(),
            "cleanup" => a.cleanup(),
            _ => Err(Error::Invalid("Unknown security action".into())),
        };
        // Password is request-scoped and is never persisted or returned.

        result?;
        Ok(a.security())
    })
    .await
    .map(Json)
}

#[cfg(test)]
mod extension_service_tests {
    use super::*;
    #[test]
    fn guest_service_rejects_identity_injection_and_checks_current_grants() {
        let dir = tempfile::tempdir().unwrap();
        let accounts = Accounts::open(
            MailStore::open(dir.path()).unwrap(),
            Arc::new(UnavailableSecrets),
        )
        .unwrap();
        let mut extensions = extensions::Extensions::open(dir.path()).unwrap();
        extensions
            .install(
                include_str!("../../../extensions/examples/connection-panel/extension.toml"),
                wat::parse_str("(module (func (export \"run\")))").unwrap(),
            )
            .unwrap();
        extensions
            .update(
                "connection-panel",
                "demo",
                true,
                vec![Capability::AccountRead],
            )
            .unwrap();
        let host = Host {
            accounts: Arc::new(Mutex::new(accounts)),
            extensions: Arc::new(Mutex::new(extensions)),
            authority: "127.0.0.1:4173".into(),
            origin: "http://127.0.0.1:4173".into(),
            execution: Arc::new(tokio::sync::Semaphore::new(2)),
        };
        let epoch = host
            .extensions
            .lock()
            .unwrap()
            .module("connection-panel")
            .unwrap()
            .0;
        let valid = serde_json::json!({"operation":"account.read"});
        assert_eq!(
            extension_service(&host, "connection-panel", "demo", epoch, valid.clone()).unwrap(),
            false
        );
        for invalid in [
            serde_json::json!({"operation":"account.read", "account":"other"}),
            serde_json::json!({"operation":"account.read", "extension":"host"}),
            serde_json::json!({"operation":"accounts.manage"}),
            serde_json::json!({"operation":"messages.read", "id":"1"}),
            serde_json::json!({"operation":"messages.label", "label":{"name":"Work", "remove":false}}),
        ] {
            assert!(extension_service(&host, "connection-panel", "demo", epoch, invalid).is_err());
        }
        assert!(
            extension_service(&host, "connection-panel", "other", epoch, valid.clone()).is_err()
        );
        host.extensions
            .lock()
            .unwrap()
            .update("connection-panel", "demo", true, vec![])
            .unwrap();
        assert!(extension_service(&host, "connection-panel", "demo", epoch, valid).is_err());
    }
}
