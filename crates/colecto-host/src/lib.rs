//! Trusted HTTP adapter for bundled UI and isolated Wasm extensions.
pub mod api_keys;
pub mod approvals;
pub mod mcp;
pub mod preferences;
mod store;
use api_keys::{ApiKeys, Principal};
mod extensions;
pub mod logging;
pub mod notifications;
mod overlay;
pub mod package;
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
    ExtensionPolicyInput, ExtensionRunInput, ExtensionState, Message, MessageContent, SecurityInfo,
    SecurityInput,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tower_http::services::ServeDir;

/// Looks up a request path such as `/index.html`, returning its bytes and MIME type.
pub type AssetLookup = Arc<dyn Fn(&str) -> Option<(Vec<u8>, String)> + Send + Sync>;

/// Where the bundled UI comes from: a directory on disk, or files compiled into the app.
#[derive(Clone)]
pub enum Assets {
    Directory(PathBuf),
    Embedded(AssetLookup),
}
impl From<PathBuf> for Assets {
    fn from(directory: PathBuf) -> Self {
        Self::Directory(directory)
    }
}

/// Named server-sent events for connected clients: `mail`, `approval` and `navigate`.
/// An empty payload closes every stream.
pub(crate) type Events = tokio::sync::broadcast::Sender<(&'static str, String)>;

#[derive(Clone)]
struct Host {
    events: Events,
    keys: Arc<Mutex<ApiKeys>>,
    preferences: Arc<Mutex<preferences::PreferenceStore>>,
    approvals: Arc<Mutex<approvals::Approvals>>,
    status_polls: Arc<Mutex<std::collections::HashMap<String, std::time::Instant>>>,
    accounts: Arc<Mutex<Accounts>>,
    extensions: Arc<Mutex<extensions::Extensions>>,
    authority: String,
    origin: String,
    execution: Arc<tokio::sync::Semaphore>,
}

struct Failure(StatusCode, String);
impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        let category = self.category();
        (
            self.0,
            Json(ApiError {
                error: self.1,
                category,
            }),
        )
            .into_response()
    }
}
impl Failure {
    fn category(&self) -> colecto_extension_api::ErrorCategory {
        use colecto_extension_api::ErrorCategory::*;
        if self.1.contains("uncertain outcome") {
            DeliveryUncertain
        } else if self.1.starts_with("Mail server rejected authentication")
            || self.1.starts_with("JMAP HTTP 401:")
            || self.1.starts_with("JMAP HTTP 403:")
        {
            ProviderAuthentication
        } else if self.1.starts_with("Mail server timed out") {
            ProviderTimeout
        } else if self.1.starts_with("Mail server TLS") {
            ProviderTls
        } else if self.1.starts_with("JMAP HTTP 429:") {
            ProviderRateLimited
        } else {
            match self.0 {
                StatusCode::UNAUTHORIZED => Authentication,
                StatusCode::FORBIDDEN => Forbidden,
                StatusCode::BAD_REQUEST => InvalidRequest,
                StatusCode::NOT_FOUND => NotFound,
                StatusCode::BAD_GATEWAY => Provider,
                StatusCode::CONFLICT => Conflict,
                _ => StorageOrService,
            }
        }
    }
}
impl From<Error> for Failure {
    fn from(error: Error) -> Self {
        match error {
            Error::Transport(_) => Self(StatusCode::BAD_GATEWAY, error.to_string()),
            Error::Missing => Self(StatusCode::NOT_FOUND, error.to_string()),
            Error::Invalid(_) => Self(StatusCode::BAD_REQUEST, error.to_string()),
            Error::Storage(_) => {
                tracing::error!(%error, "Local mail storage error");
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

fn build_app(host: Host, assets: Assets) -> Router {
    let api = Router::new()
        .route(
            "/preferences",
            get(get_preferences).post(update_preferences),
        )
        .route("/api-keys", get(list_api_keys).post(mint_api_key))
        .route("/api-keys/{id}", axum::routing::delete(revoke_api_key))
        .route(
            "/extensions",
            get(list_extensions)
                .post(install_extension)
                .layer(DefaultBodyLimit::max(package::MAX_UPLOAD_BODY)),
        )
        .route(
            "/extensions/{id}",
            axum::routing::post(update_extension).delete(remove_extension),
        )
        .route("/extensions/{id}/assets/{name}", get(extension_asset))
        .route(
            "/extensions/review",
            axum::routing::post(review_extension)
                .layer(DefaultBodyLimit::max(package::MAX_UPLOAD_BODY)),
        )
        .route("/extensions/{id}/run", axum::routing::post(run_extension))
        .route("/accounts", get(list_accounts).post(upsert_account))
        .route(
            "/accounts/{id}/disconnect",
            axum::routing::post(disconnect_account),
        )
        .route("/security", get(security).post(security_action))
        .route("/account", get(account))
        // Base64 attachments up to 25 MB, plus the message text.
        .route(
            "/send",
            axum::routing::post(send).layer(DefaultBodyLimit::max(36_000_000)),
        )
        .route("/send-requests", get(list_send_requests))
        .route("/send-requests/{id}", get(send_request))
        .route(
            "/send-requests/{id}/approve",
            axum::routing::post(approve_send),
        )
        .route(
            "/send-requests/{id}/reject",
            axum::routing::post(reject_send),
        )
        .route("/labels", get(labels).post(update_label))
        .route("/messages", get(messages))
        .route("/messages/cache", get(cached_messages))
        .route("/messages/refresh", axum::routing::post(messages))
        .route("/messages/{id}", get(message))
        .route("/messages/{id}/content", get(message_content))
        .route(
            "/messages/{id}/attachments/{index}",
            get(message_attachment),
        )
        .route("/drafts", get(drafts).post(save_draft))
        .route("/drafts/{id}", get(draft))
        .fallback(|| async { Failure(StatusCode::NOT_FOUND, "API route not found".into()) });
    let router = Router::new().nest("/api/v1", api);
    let router = match assets {
        Assets::Directory(directory) => router.fallback_service(ServeDir::new(directory)),
        Assets::Embedded(lookup) => router.fallback(move |uri: axum::http::Uri| async move {
            match lookup(uri.path()) {
                Some((body, mime)) => ([(header::CONTENT_TYPE, mime)], body).into_response(),
                None => StatusCode::NOT_FOUND.into_response(),
            }
        }),
    };
    router
        .layer(DefaultBodyLimit::max(512_000))
        // Path only: query strings can carry search terms, which stay out of logs.
        .layer(
            tower_http::trace::TraceLayer::new_for_http().make_span_with(
                |request: &axum::http::Request<_>| {
                    tracing::debug_span!("request", method = %request.method(), path = request.uri().path())
                },
            ),
        )
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
        None => !mutation || request.uri().path() == "/api/v1/mcp",
    };
    let mut response = if !authority_ok || !origin_ok {
        forbidden().into_response()
    } else {
        next.run(request).await
    };
    let headers = response.headers_mut();
    headers.entry(header::CONTENT_SECURITY_POLICY).or_insert(
        HeaderValue::from_static(
            // Email HTML renders in a script-free sandboxed srcdoc frame that inherits
            // this policy; the frame adds its own stricter policy for remote images.
            "default-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: https:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'",
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
fn grant(
    host: &Host,
    principal: &Principal,
    headers: &HeaderMap,
    operation: Capability,
) -> Result<String, Failure> {
    let account = headers
        .get("x-colecto-account")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(forbidden)?;
    if let Principal::ApiKey(id) = principal {
        return if host.keys.lock().map_err(|_| forbidden())?.allows(
            id,
            account,
            operation,
            api_keys::now(),
        ) {
            Ok(account.to_owned())
        } else {
            Err(forbidden())
        };
    }
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
    extension_account(&host, &headers)?;
    let Json(input) =
        input.map_err(|_| Failure(StatusCode::BAD_REQUEST, "Invalid extension policy".into()))?;
    host.extensions
        .lock()
        .map_err(|_| forbidden())?
        .update(&id, input.enabled)
        .map_err(|error| {
            if error == "Unknown extension" { forbidden() }
            else { Failure(StatusCode::INTERNAL_SERVER_ERROR, "Could not confirm extension policy persistence. Reload permissions before retrying.".into()) }
        })?;
    Ok(Json(true))
}

fn extension_upload(headers: &HeaderMap, body: axum::body::Bytes) -> Result<Vec<u8>, Failure> {
    if headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/json"))
    {
        serde_json::from_slice::<ExtensionInstallInput>(&body)
            .map(|input| input.package)
            .map_err(|_| Failure(StatusCode::BAD_REQUEST, "Invalid extension package".into()))
    } else if body.len() <= package::MAX_PACKAGE {
        Ok(body.to_vec())
    } else {
        Err(Failure(
            StatusCode::PAYLOAD_TOO_LARGE,
            "Extension package must be at most 16 MiB".into(),
        ))
    }
}
async fn install_extension(
    State(host): State<Host>,
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Json<bool>, Failure> {
    host_grant(&headers)?;
    let bytes = extension_upload(&headers, body)?;
    tokio::task::spawn_blocking(move || {
        host.extensions
            .lock()
            .map_err(|_| forbidden())?
            .install_package(&bytes)
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
async fn review_extension(
    headers: HeaderMap,
    body: axum::body::Bytes,
) -> Result<Json<colecto_extension_api::ExtensionManifest>, Failure> {
    host_grant(&headers)?;
    let bytes = extension_upload(&headers, body)?;
    tokio::task::spawn_blocking(move || {
        let mut extensions = extensions::Extensions::bundled();
        extensions
            .install_package(&bytes)
            .map_err(|e| Failure(StatusCode::BAD_REQUEST, e))?;
        Ok(Json(
            extensions
                .list("")
                .into_iter()
                .find(|e| !e.bundled)
                .ok_or_else(forbidden)?
                .manifest,
        ))
    })
    .await
    .map_err(|_| {
        Failure(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Extension review failed".into(),
        )
    })?
}
async fn extension_asset(
    State(host): State<Host>,
    headers: HeaderMap,
    Path((id, name)): Path<(String, String)>,
) -> Result<impl IntoResponse, Failure> {
    host_grant(&headers)?;
    let assets = host
        .extensions
        .lock()
        .map_err(|_| forbidden())?
        .assets(&id)
        .ok_or_else(forbidden)?;
    let bytes = assets.get(&name).ok_or_else(forbidden)?.clone();
    let content_type = package::asset_content_type(&name, &bytes).ok_or_else(forbidden)?;
    Ok((
        [
            ("content-type", content_type),
            ("x-content-type-options", "nosniff"),
            ("content-security-policy", "sandbox; default-src 'none'"),
        ],
        bytes,
    ))
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
) -> Result<Json<serde_json::Value>, Failure> {
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
    let assets = {
        let extensions = host.extensions.lock().map_err(|_| forbidden())?;
        if extensions.overlay(&id) {
            Some(extensions.assets(&id).ok_or_else(forbidden)?)
        } else {
            None
        }
    };
    let permit = host.execution.clone().try_acquire_owned().map_err(|_| {
        Failure(
            StatusCode::TOO_MANY_REQUESTS,
            "Extensions busy; try again".into(),
        )
    })?;
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        sandbox::run_output(
            &module,
            &input.action,
            Box::new(move |request| extension_service(&host, &id, &account, epoch, request)),
            assets,
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
    if request.operation == Capability::MessagesRead
        && let Some(message) = host
            .accounts
            .lock()
            .map_err(|_| "Account service unavailable")?
            .cached_message(account, request.id.as_deref().ok_or("Message ID required")?)
            .map_err(|_| "Mail cache unavailable")?
    {
        return serde_json::to_value(message)
            .map_err(|_| "Cannot serialize service response".into());
    }
    if request.operation == Capability::MessagesList {
        return serde_json::to_value(
            host.accounts
                .lock()
                .map_err(|_| "Account service unavailable")?
                .cached(account, "inbox")
                .map_err(|_| "Mail cache unavailable")?
                .messages,
        )
        .map_err(|_| "Cannot serialize service response".into());
    }
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
        AppNavigate | WindowOverlay => return Err("Capability not granted".into()),
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
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
) -> Result<Json<std::collections::BTreeMap<String, Vec<String>>>, Failure> {
    let account = grant(&host, &principal, &headers, Capability::MessagesList)?;
    with_store(host, account, |s| s.labels()).await.map(Json)
}
async fn update_label(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    input: Result<Json<colecto_extension_api::LabelInput>, JsonRejection>,
) -> Result<Json<std::collections::BTreeMap<String, Vec<String>>>, Failure> {
    let account = grant(&host, &principal, &headers, Capability::MessagesLabel)?;
    let Json(input) = input.map_err(|error| Failure(StatusCode::BAD_REQUEST, error.body_text()))?;
    with_store(host, account, move |s| {
        s.update_label(&input.name, input.message_id.as_deref(), input.remove)
    })
    .await
    .map(Json)
}
fn redact_list_bodies(
    host: &Host,
    principal: &Principal,
    account: &str,
    messages: &mut [Message],
) -> Result<(), Failure> {
    if let Principal::ApiKey(id) = principal
        && !host.keys.lock().map_err(|_| forbidden())?.allows(
            id,
            account,
            Capability::MessagesRead,
            api_keys::now(),
        )
    {
        for message in messages {
            message.body.clear();
        }
    }
    Ok(())
}
async fn cached_messages(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Query(query): Query<MessageQuery>,
) -> Result<Json<colecto_core::MailSnapshot>, Failure> {
    let account = grant(&host, &principal, &headers, Capability::MessagesList)?;
    if !query.q.is_empty() {
        return Err(Failure(
            StatusCode::BAD_REQUEST,
            "Cached snapshots do not run provider searches".into(),
        ));
    }
    let mut snapshot = host
        .accounts
        .lock()
        .map_err(|_| forbidden())?
        .cached(&account, &query.folder)
        .map_err(Failure::from)?;
    redact_list_bodies(&host, &principal, &account, &mut snapshot.messages)?;
    Ok(Json(snapshot))
}
async fn messages(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Query(query): Query<MessageQuery>,
) -> Result<Json<Vec<Message>>, Failure> {
    let account = grant(&host, &principal, &headers, Capability::MessagesList)?;
    let mut messages = with_store(host.clone(), account.clone(), move |s| {
        s.query(&query.folder, &query.q)
    })
    .await?;
    redact_list_bodies(&host, &principal, &account, &mut messages)?;
    Ok(Json(messages))
}
async fn message(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Message>, Failure> {
    let account = grant(&host, &principal, &headers, Capability::MessagesRead)?;
    if let Some(message) = host
        .accounts
        .lock()
        .map_err(|_| forbidden())?
        .cached_message(&account, &id)
        .map_err(Failure::from)?
    {
        return Ok(Json(message));
    }
    with_store(host, account, move |s| {
        s.message(&id).cloned().ok_or(Error::Missing)
    })
    .await
    .map(Json)
}
async fn message_content(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<MessageContent>, Failure> {
    let account = grant(&host, &principal, &headers, Capability::MessagesRead)?;
    with_store(host, account, move |s| s.content(&id))
        .await
        .map(Json)
}
async fn message_attachment(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Path((id, index)): Path<(String, usize)>,
) -> Result<Response, Failure> {
    let account = grant(&host, &principal, &headers, Capability::MessagesRead)?;
    let (info, data) = with_store(host, account, move |s| s.attachment(&id, index)).await?;
    let name: String = info
        .name
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"._-".contains(&byte) {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect();
    // Never let the browser render untrusted attachment bytes in the app origin.
    Ok((
        [
            (header::CONTENT_TYPE, "application/octet-stream".to_owned()),
            (
                header::CONTENT_DISPOSITION,
                format!("attachment; filename*=UTF-8''{name}"),
            ),
        ],
        data,
    )
        .into_response())
}
async fn drafts(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
) -> Result<Json<Vec<Draft>>, Failure> {
    let account = grant(&host, &principal, &headers, Capability::DraftsList)?;
    let mut drafts = with_store(host.clone(), account.clone(), |s| Ok(s.drafts().to_vec())).await?;
    if let Principal::ApiKey(id) = &principal
        && !host.keys.lock().map_err(|_| forbidden())?.allows(
            id,
            &account,
            Capability::DraftsRead,
            api_keys::now(),
        )
    {
        for draft in &mut drafts {
            draft.body.clear();
        }
    }
    Ok(Json(drafts))
}
async fn draft(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Draft>, Failure> {
    let account = grant(&host, &principal, &headers, Capability::DraftsRead)?;
    with_store(host, account, move |s| {
        s.draft(&id).cloned().ok_or(Error::Missing)
    })
    .await
    .map(Json)
}
async fn save_draft(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    input: Result<Json<DraftInput>, JsonRejection>,
) -> Result<Json<Draft>, Failure> {
    let account = grant(&host, &principal, &headers, Capability::DraftsWrite)?;
    let Json(input) = input.map_err(|error| Failure(StatusCode::BAD_REQUEST, error.body_text()))?;
    with_store(host, account, move |s| s.save_draft(input))
        .await
        .map(Json)
}

async fn account(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
) -> Result<Json<bool>, Failure> {
    let account = grant(&host, &principal, &headers, Capability::AccountRead)?;
    with_store(host, account, |s| Ok(s.connected()))
        .await
        .map(Json)
}
async fn send(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    input: Result<Json<DraftInput>, JsonRejection>,
) -> Result<Json<serde_json::Value>, Failure> {
    let account = grant(&host, &principal, &headers, Capability::MessagesSend)?;
    let Json(input) = input.map_err(|error| Failure(StatusCode::BAD_REQUEST, error.body_text()))?;
    // Forwarding re-reads the original message, so it needs read access too.
    if input.forward_of.is_some() {
        grant(&host, &principal, &headers, Capability::MessagesRead)?;
    }
    if let Principal::ApiKey(id) = principal {
        return request_delivery(host, id, account, input).await.map(Json);
    }
    with_store(host, account, move |s| {
        s.send(input)?;
        Ok(serde_json::json!(true))
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

async fn get_preferences(
    State(host): State<Host>,
    headers: HeaderMap,
) -> Result<Json<preferences::Preferences>, Failure> {
    host_grant(&headers)?;
    Ok(Json(
        host.preferences
            .lock()
            .map_err(|_| forbidden())?
            .get()
            .clone(),
    ))
}
async fn update_preferences(
    State(host): State<Host>,
    headers: HeaderMap,
    input: Result<Json<preferences::PreferencesInput>, JsonRejection>,
) -> Result<Json<preferences::Preferences>, Failure> {
    host_grant(&headers)?;
    let Json(input) =
        input.map_err(|_| Failure(StatusCode::BAD_REQUEST, "Invalid preferences".into()))?;
    tokio::task::spawn_blocking(move || {
        host.preferences
            .lock()
            .map_err(|_| forbidden())?
            .update(input)
            .map_err(|e| Failure(StatusCode::BAD_REQUEST, e))
    })
    .await
    .map_err(|_| forbidden())?
    .map(Json)
}
async fn list_api_keys(
    State(host): State<Host>,
    headers: HeaderMap,
) -> Result<Json<Vec<api_keys::KeyMetadata>>, Failure> {
    host_grant(&headers)?;
    Ok(Json(host.keys.lock().map_err(|_| forbidden())?.list()))
}
async fn mint_api_key(
    State(host): State<Host>,
    headers: HeaderMap,
    input: Result<Json<api_keys::KeyInput>, JsonRejection>,
) -> Result<Json<serde_json::Value>, Failure> {
    host_grant(&headers)?;
    let Json(input) =
        input.map_err(|_| Failure(StatusCode::BAD_REQUEST, "Invalid API key settings".into()))?;
    let accounts = host.accounts.lock().map_err(|_| forbidden())?.list();
    if input
        .accounts
        .iter()
        .any(|id| !accounts.iter().any(|a| &a.id == id))
    {
        return Err(forbidden());
    }
    let (key, secret) = host
        .keys
        .lock()
        .map_err(|_| forbidden())?
        .mint(input, api_keys::now())
        .map_err(|e| Failure(StatusCode::BAD_REQUEST, e))?;
    Ok(Json(serde_json::json!({"key":key,"secret":secret})))
}
async fn revoke_api_key(
    State(host): State<Host>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<bool>, Failure> {
    host_grant(&headers)?;
    host.keys
        .lock()
        .map_err(|_| forbidden())?
        .revoke(&id, api_keys::now())
        .map_err(|e| Failure(StatusCode::BAD_REQUEST, e))?;
    Ok(Json(true))
}

fn approval_failure(error: String) -> Failure {
    Failure(StatusCode::CONFLICT, error)
}
fn require_owner(principal: &Principal, headers: &HeaderMap) -> Result<(), Failure> {
    if !matches!(principal, Principal::Owner) {
        return Err(forbidden());
    }
    host_grant(headers)
}
/// Ask every connected client to open `target`; mail pages also select `account`.
fn navigate(
    host: &Host,
    principal: &Principal,
    account: &str,
    target: &str,
) -> Result<serde_json::Value, Failure> {
    let Principal::ApiKey(key) = principal else {
        return Err(forbidden());
    };
    if !host.keys.lock().map_err(|_| forbidden())?.allows(
        key,
        account,
        Capability::AppNavigate,
        api_keys::now(),
    ) {
        return Err(forbidden());
    }
    let event = serde_json::json!({"target":target,"account":account}).to_string();
    let clients = host.events.send(("navigate", event)).unwrap_or(0);
    Ok(serde_json::json!({"target":target,"clients":clients}))
}

async fn request_delivery(
    host: Host,
    key: String,
    account: String,
    input: DraftInput,
) -> Result<serde_json::Value, Failure> {
    tokio::task::spawn_blocking(move || {
        let accounts = host.accounts.lock().map_err(|_| forbidden())?;
        let info = accounts.list().into_iter()
            .find(|a| a.id == account && a.connected)
            .ok_or_else(forbidden)?;
        let from = info.settings.ok_or_else(forbidden)?.address;
        let mut keys = host.keys.lock().map_err(|_| forbidden())?;
        if !keys.allows(&key, &account, Capability::MessagesSend, api_keys::now()) {
            return Err(forbidden());
        }
        let subject = input.subject.clone();
        let request = host.approvals.lock().map_err(|_| forbidden())?
            .create(&key, &account, &from, input, api_keys::now())
            .map_err(approval_failure)?;
        // Connected clients offer Approve and Reject in a notification; the decision still goes through the owner-only route.
        let _ = host.events.send(("approval", serde_json::json!({"id":request.id,"account":account,"from":from,"subject":subject}).to_string()));
        Ok(serde_json::json!({"id":request.id,"status":request.status,"expires_at":request.expires_at}))
    }).await.map_err(|_| approval_failure("Delivery request service unavailable".into()))?
}

async fn list_send_requests(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
) -> Result<Json<Vec<approvals::PendingSend>>, Failure> {
    require_owner(&principal, &headers)?;
    Ok(Json(
        host.approvals
            .lock()
            .map_err(|_| forbidden())?
            .list(api_keys::now())
            .map_err(approval_failure)?,
    ))
}
async fn request_status(
    host: &Host,
    principal: &Principal,
    id: &str,
    account: Option<&str>,
) -> Result<serde_json::Value, Failure> {
    if let Principal::ApiKey(key) = principal {
        let mut polls = host.status_polls.lock().map_err(|_| forbidden())?;
        if polls
            .get(key)
            .is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(1))
        {
            return Err(Failure(
                StatusCode::TOO_MANY_REQUESTS,
                "Poll at most once per second".into(),
            ));
        }
        if polls.len() >= 128 {
            polls.retain(|_, t| t.elapsed() < std::time::Duration::from_secs(1));
        }
        polls.insert(key.clone(), std::time::Instant::now());
    }
    let request = host
        .approvals
        .lock()
        .map_err(|_| forbidden())?
        .get(id, api_keys::now())
        .map_err(|_| Failure(StatusCode::NOT_FOUND, "Delivery request not found".into()))?;
    if account.is_some_and(|account| request.account != account) {
        return Err(Failure(
            StatusCode::NOT_FOUND,
            "Delivery request not found".into(),
        ));
    }
    if let Principal::ApiKey(key) = principal
        && (request.requester != *key
            || !host.keys.lock().map_err(|_| forbidden())?.allows(
                key,
                &request.account,
                Capability::MessagesSend,
                api_keys::now(),
            ))
    {
        return Err(Failure(
            StatusCode::NOT_FOUND,
            "Delivery request not found".into(),
        ));
    }
    Ok(serde_json::json!({"id":request.id,"status":request.status,"expires_at":request.expires_at}))
}
async fn send_request(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, Failure> {
    request_status(&host, &principal, &id, None).await.map(Json)
}
async fn reject_send(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<approvals::PendingSend>, Failure> {
    require_owner(&principal, &headers)?;
    host.approvals
        .lock()
        .map_err(|_| forbidden())?
        .reject(&id, api_keys::now())
        .map(Json)
        .map_err(approval_failure)
}
async fn approve_send(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<approvals::PendingSend>, Failure> {
    require_owner(&principal, &headers)?;
    let permit = host.execution.clone().try_acquire_owned().map_err(|_| {
        Failure(
            StatusCode::TOO_MANY_REQUESTS,
            "Mail service busy; try again".into(),
        )
    })?;
    // This worker survives an HTTP client disconnect. Persist submitting before touching transport.
    tokio::task::spawn_blocking(move || {
        let _permit = permit;
        let accounts = host.accounts.lock().map_err(|_| forbidden())?;
        let request = host
            .approvals
            .lock()
            .map_err(|_| forbidden())?
            .get(&id, api_keys::now())
            .map_err(approval_failure)?;
        let info = accounts
            .list()
            .into_iter()
            .find(|a| {
                a.id == request.account
                    && a.connected
                    && a.settings
                        .as_ref()
                        .is_some_and(|s| s.address == request.from)
            })
            .ok_or_else(forbidden)?;
        let store = accounts.store(&info.id).map_err(Failure::from)?;
        let store = store.lock().map_err(|_| forbidden())?;
        let mut keys = host.keys.lock().map_err(|_| forbidden())?;
        if !keys.allows(
            &request.requester,
            &request.account,
            Capability::MessagesSend,
            api_keys::now(),
        ) {
            return Err(forbidden());
        }
        let snapshot = host
            .approvals
            .lock()
            .map_err(|_| forbidden())?
            .begin(&id, api_keys::now())
            .map_err(approval_failure)?;
        drop(keys);
        drop(accounts);
        let sent = store.send(snapshot).is_ok();
        let mut approvals = host.approvals.lock().map_err(|_| forbidden())?;
        approvals.finish(&id, sent).map_err(approval_failure)?;
        approvals
            .get(&id, api_keys::now())
            .map(Json)
            .map_err(approval_failure)
    })
    .await
    .map_err(|_| approval_failure("Delivery outcome uncertain; do not retry".into()))?
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
        let database = accounts.database();
        let mut extensions = extensions::Extensions::open(database.clone(), dir.path()).unwrap();
        extensions
            .install(
                include_str!("../../../docs/extensions/examples/connection-panel/extension.toml"),
                wat::parse_str("(module (func (export \"run\")))").unwrap(),
            )
            .unwrap();
        extensions.update("connection-panel", true).unwrap();
        let host = Host {
            events: tokio::sync::broadcast::channel(1).0,
            keys: Arc::new(Mutex::new(
                ApiKeys::open(database.clone(), dir.path()).unwrap(),
            )),
            preferences: Arc::new(Mutex::new(
                preferences::PreferenceStore::open(database.clone(), dir.path()).unwrap(),
            )),
            approvals: Arc::new(Mutex::new(
                approvals::Approvals::open(database, dir.path(), api_keys::now()).unwrap(),
            )),
            status_polls: Arc::default(),
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
            .update("connection-panel", false)
            .unwrap();
        assert!(extension_service(&host, "connection-panel", "demo", epoch, valid).is_err());
    }
}
