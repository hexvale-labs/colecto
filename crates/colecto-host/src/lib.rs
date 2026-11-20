//! Trusted HTTP adapter for the UI shell, extension client parts and Wasm service parts.
mod ai;
pub mod api_keys;
pub mod approvals;
mod bus;
mod credentials;
mod discovery;
pub mod mcp;
pub mod preferences;
mod releases;
pub mod services;
mod settings;
mod store;
mod sync;
use api_keys::{ApiKeys, Principal};
mod extensions;
pub mod logging;
pub mod notifications;
pub use colecto_package as package;
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
    ExtensionPolicyInput, ExtensionState, Message, MessageContent, SecurityInfo, SecurityInput,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tower_http::services::ServeDir;

/// This build's version: the workspace `version` in the root `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The Git commit this build came from, when the build sets `COLECTO_COMMIT`
/// (release CI, the Docker image and the AUR `-git` package do).
pub fn commit() -> Option<&'static str> {
    option_env!("COLECTO_COMMIT").filter(|commit| !commit.is_empty())
}

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

/// Named server-sent events for connected clients: `mail`, `approval`, `navigate` and `sync`.
/// An empty payload closes every stream.
pub(crate) type Events = tokio::sync::broadcast::Sender<(&'static str, String)>;

#[derive(Clone)]
struct Host {
    events: Events,
    bus_replies: bus::Replies,
    keys: Arc<Mutex<ApiKeys>>,
    preferences: Arc<Mutex<preferences::PreferenceStore>>,
    approvals: Arc<Mutex<approvals::Approvals>>,
    status_polls: Arc<Mutex<std::collections::HashMap<String, std::time::Instant>>>,
    accounts: Arc<Mutex<Accounts>>,
    extensions: Arc<Mutex<extensions::Extensions>>,
    authority: String,
    origin: String,
    execution: Arc<tokio::sync::Semaphore>,
    ai: Arc<ai::Ai>,
    sync: Arc<sync::SyncState>,
    credentials: Arc<Mutex<credentials::Credentials>>,
    /// Open Colecto windows (shell event streams). While one is open, the background
    /// notifier leaves new-mail notifications to it.
    windows: Arc<std::sync::atomic::AtomicUsize>,
}

/// Counts an open window for as long as its event stream lives.
struct Window(Arc<std::sync::atomic::AtomicUsize>);
impl Window {
    fn open(count: &Arc<std::sync::atomic::AtomicUsize>) -> Self {
        count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self(count.clone())
    }
}
impl Drop for Window {
    fn drop(&mut self) {
        self.0.fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
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
            "/extensions/{id}/settings",
            get(get_extension_settings).put(put_extension_settings),
        )
        .route(
            "/extensions/{id}/settings/reset",
            axum::routing::post(reset_extension_settings),
        )
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
        .route("/bus/{bus}/request", axum::routing::post(bus::request_http))
        .route("/bus/events", axum::routing::post(bus::emit_http))
        .route("/bus/replies/{id}", axum::routing::post(bus::reply_http))
        .route(
            "/extensions/{id}/credential",
            axum::routing::post(mint_credential).delete(revoke_credential),
        )
        .route("/extensions/{id}/client", get(client_files))
        .route(
            "/extensions/{id}/storage/{key}",
            get(get_storage).put(put_storage),
        )
        .route("/extensions/{id}/client/{*path}", get(client_file))
        .route("/extensions/events", axum::routing::post(extension_events))
        .route("/accounts", get(list_accounts).post(upsert_account))
        .route("/accounts/discover", axum::routing::post(discover_account))
        .route("/accounts/{id}", axum::routing::delete(remove_account))
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
        .route("/folders", get(folders))
        .route("/messages", get(messages))
        .route("/messages/cache", get(cached_messages))
        .route("/messages/refresh", axum::routing::post(messages))
        .route("/messages/{id}", get(message).post(organize_messages))
        .route("/messages/{id}/content", get(message_content))
        .route("/sync", get(sync_status))
        .route(
            "/messages/{id}/attachments/{index}",
            get(message_attachment),
        )
        .route("/drafts", get(drafts).post(save_draft))
        .route("/drafts/{id}", get(draft))
        .route("/version", get(version))
        .route("/releases/latest", get(latest_release))
        .route("/ai/settings", get(ai::get_settings).post(ai::put_settings))
        .route("/ai/keys", axum::routing::post(ai::put_key))
        .route("/ai/test", axum::routing::post(ai::test_connection))
        .route("/ai/labels/preview", axum::routing::post(ai::preview))
        .route(
            "/ai/summaries/{id}",
            get(ai::get_summary)
                .post(ai::create_summary)
                .delete(ai::delete_summary),
        )
        .route("/ai/reply", axum::routing::post(ai::draft_reply))
        .fallback(|| async { Failure(StatusCode::NOT_FOUND, "API route not found".into()) });
    let router = Router::new()
        .nest("/api/v1", api)
        .route("/extension-runtime.html", get(extension_runtime));
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

/// The page each extension's client part starts in, inside an opaque-origin sandboxed frame.
const RUNTIME_PAGE: &str = include_str!("../../../ui/src/runtime/frame.html");
/// Its policy allows exactly its own inline script (by hash), the Worker it starts from a
/// blob, and Wasm; no network, images, styles or forms. Blob Workers inherit it.
fn runtime_policy() -> &'static str {
    use sha2::Digest;
    static POLICY: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    POLICY.get_or_init(|| {
        let script = RUNTIME_PAGE
            .split_once("<script>")
            .and_then(|(_, rest)| rest.split_once("</script>"))
            .map(|(script, _)| script)
            .expect("runtime page has one inline script");
        let hash = base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            sha2::Sha256::digest(script.as_bytes()),
        );
        format!(
            "default-src 'none'; script-src 'sha256-{hash}' blob: 'wasm-unsafe-eval'; worker-src blob:; connect-src 'none'; img-src 'none'; style-src 'none'; font-src 'none'; media-src 'none'; object-src 'none'; frame-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'self'"
        )
    })
}
async fn extension_runtime() -> impl IntoResponse {
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CONTENT_SECURITY_POLICY, runtime_policy()),
        ],
        RUNTIME_PAGE,
    )
}

/// Public, like a health check: callers can check compatibility before they hold a token.
async fn version() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "name": "colecto",
        "version": VERSION,
        "commit": commit(),
        "api": "v1",
    }))
}

/// Owner only: a newer stable release, or `null`. The UI calls it once per start, and only
/// when the owner keeps update notifications on, so turning them off means no request.
async fn latest_release(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, Failure> {
    if let Principal::Extension(token) = &principal {
        let (id, generation) = caller_extension(&host, token)?;
        let extensions = host.extensions.lock().map_err(|_| forbidden())?;
        if !extensions.allows_generation(&id, "", Capability::ReleasesRead, generation) {
            return Err(forbidden());
        }
    } else {
        host_grant(&headers)?;
    }
    let latest = tokio::task::spawn_blocking(|| releases::latest(VERSION))
        .await
        .map_err(|_| forbidden())?
        .map_err(|error| Failure(StatusCode::BAD_GATEWAY, error))?;
    Ok(Json(
        serde_json::json!({ "current": VERSION, "latest": latest }),
    ))
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
    if let Principal::Extension(token) = principal {
        // A client part: its credential names the extension; headers name only the account,
        // and work on an account that does not exist fails where it is done, as for the owner.
        let (id, generation) = caller_extension(host, token)?;
        return if host
            .extensions
            .lock()
            .map_err(|_| forbidden())?
            .allows_generation(&id, account, operation, generation)
        {
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
            else if error.starts_with("This extension") { Failure(StatusCode::BAD_REQUEST, error) }
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
) -> Result<Json<colecto_extension_api::ExtensionReview>, Failure> {
    host_grant(&headers)?;
    let bytes = extension_upload(&headers, body)?;
    tokio::task::spawn_blocking(move || {
        let bad = |e| Failure(StatusCode::BAD_REQUEST, e);
        // A discontinued API version is reported, not refused, so the UI can explain why.
        let source = package::read(&bytes).map_err(bad)?.manifest;
        let manifest = package::manifest::parse_any_version(&source).map_err(bad)?;
        if let colecto_extension_api::ApiSupport::Discontinued(release) =
            colecto_extension_api::api_support(manifest.api_version)
        {
            return Ok(Json(colecto_extension_api::ExtensionReview {
                manifest,
                discontinued: Some(release.into()),
            }));
        }
        let mut extensions = extensions::Extensions::bundled();
        extensions
            .install_package(&bytes)
            .map_err(|e| Failure(StatusCode::BAD_REQUEST, e))?;
        Ok(Json(
            extensions
                .list("")
                .into_iter()
                .find(|e| !e.bundled)
                .map(|e| colecto_extension_api::ExtensionReview {
                    manifest: e.manifest,
                    discontinued: None,
                })
                .ok_or_else(forbidden)?,
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
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Path((id, name)): Path<(String, String)>,
) -> Result<impl IntoResponse, Failure> {
    // The owner, or the extension's own client part (Oneko loads its sprite sheet).
    own_or_owner(&host, &principal, &headers, &id)?;
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
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StorageInput {
    value: serde_json::Value,
}
/// An extension's own store: its client part, or the owner.
async fn get_storage(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Path((id, key)): Path<(String, String)>,
) -> Result<Json<serde_json::Value>, Failure> {
    own_or_owner(&host, &principal, &headers, &id)?;
    let value = host
        .extensions
        .lock()
        .map_err(|_| forbidden())?
        .stored(&id, &key);
    Ok(Json(serde_json::json!({ "value": value })))
}
async fn put_storage(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Path((id, key)): Path<(String, String)>,
    Json(input): Json<StorageInput>,
) -> Result<Json<bool>, Failure> {
    own_or_owner(&host, &principal, &headers, &id)?;
    host.extensions
        .lock()
        .map_err(|_| forbidden())?
        .store(&id, &key, input.value)
        .map_err(|e| Failure(StatusCode::BAD_REQUEST, e))?;
    Ok(Json(true))
}
/// The owner (the shell), or the client part of extension `id` itself.
fn own_or_owner(
    host: &Host,
    principal: &Principal,
    headers: &HeaderMap,
    id: &str,
) -> Result<(), Failure> {
    match principal {
        Principal::Extension(token) if caller_extension(host, token)?.0 == id => Ok(()),
        Principal::Extension(_) | Principal::ApiKey(_) => Err(forbidden()),
        Principal::Owner => host_grant(headers),
    }
}
/// The extension a client credential belongs to, while it still runs at the generation the
/// credential was minted for.
pub(crate) fn caller_extension(host: &Host, token: &str) -> Result<(String, u64), Failure> {
    let (id, generation) = host
        .credentials
        .lock()
        .map_err(|_| forbidden())?
        .resolve(token)
        .ok_or_else(|| {
            Failure(
                StatusCode::UNAUTHORIZED,
                "Extension credential revoked".into(),
            )
        })?;
    if host
        .extensions
        .lock()
        .map_err(|_| forbidden())?
        .generation(&id)
        != Some(generation)
    {
        return Err(Failure(
            StatusCode::UNAUTHORIZED,
            "Extension credential revoked".into(),
        ));
    }
    Ok((id, generation))
}
/// Owner only: a credential for one enabled extension's client part. The shell keeps it and
/// never hands it to the extension's Worker.
async fn mint_credential(
    State(host): State<Host>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, Failure> {
    host_grant(&headers)?;
    let generation = {
        let extensions = host.extensions.lock().map_err(|_| forbidden())?;
        extensions
            .generation(&id)
            .filter(|_| extensions.has_client(&id))
            .ok_or_else(forbidden)?
    };
    let mut credentials = host.credentials.lock().map_err(|_| forbidden())?;
    // Tokens of disabled, removed or reinstalled extensions are dead weight; drop them here.
    if let Ok(extensions) = host.extensions.lock() {
        credentials.retain(|id| extensions.generation(id));
    }
    let token = credentials
        .mint(&id, generation)
        .map_err(|e| Failure(StatusCode::INTERNAL_SERVER_ERROR, e))?;
    Ok(Json(serde_json::json!({ "token": token })))
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct TokenInput {
    token: String,
}
/// Owner only: forget one credential, for example when a window closes.
async fn revoke_credential(
    State(host): State<Host>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(input): Json<TokenInput>,
) -> Result<Json<bool>, Failure> {
    host_grant(&headers)?;
    host.credentials
        .lock()
        .map_err(|_| forbidden())?
        .revoke(&id, &input.token);
    Ok(Json(true))
}
/// Owner only: the paths under an enabled third-party extension's `client/`.
async fn client_files(
    State(host): State<Host>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Vec<String>>, Failure> {
    host_grant(&headers)?;
    let files = host
        .extensions
        .lock()
        .map_err(|_| forbidden())?
        .client_files(&id)
        .ok_or_else(forbidden)?;
    Ok(Json(files.keys().cloned().collect()))
}
/// Owner only: one file under an enabled third-party extension's `client/`, for the shell to
/// post into the extension's runtime frame. Bundled client parts ship with the UI instead.
async fn client_file(
    State(host): State<Host>,
    headers: HeaderMap,
    Path((id, path)): Path<(String, String)>,
) -> Result<impl IntoResponse, Failure> {
    host_grant(&headers)?;
    let files = host
        .extensions
        .lock()
        .map_err(|_| forbidden())?
        .client_files(&id)
        .ok_or_else(forbidden)?;
    let bytes = files
        .get(&format!("{}{path}", package::CLIENT_DIR))
        .ok_or_else(|| Failure(StatusCode::NOT_FOUND, "No such client file".into()))?
        .clone();
    Ok((
        [
            ("content-type", "application/octet-stream"),
            ("x-content-type-options", "nosniff"),
            ("content-security-policy", "sandbox; default-src 'none'"),
        ],
        bytes,
    ))
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct EventsInput {
    tokens: Vec<String>,
}
/// Owner only: one event stream for every running client part. Each event names its
/// recipients (`{"to": [ids], "data": ...}`), chosen by the same manifest rules as the rest
/// of the API; an extension whose credential is revoked drops out at the next event.
async fn extension_events(
    State(host): State<Host>,
    headers: HeaderMap,
    Json(input): Json<EventsInput>,
) -> Result<impl IntoResponse, Failure> {
    use axum::response::sse::{Event, KeepAlive, Sse};
    host_grant(&headers)?;
    if input.tokens.len() > 64 {
        return Err(Failure(
            StatusCode::BAD_REQUEST,
            "Too many credentials".into(),
        ));
    }
    let receiver = host.events.subscribe();
    let tokens = input.tokens;
    let window = Window::open(&host.windows);
    let stream = futures_util::stream::unfold(
        (host, receiver, tokens, true, window),
        |(host, mut receiver, tokens, first, window)| async move {
            if first {
                let event = Event::default().event("ready").data("{}");
                return Some((
                    Ok::<_, std::convert::Infallible>(event),
                    (host, receiver, tokens, false, window),
                ));
            }
            loop {
                let (name, data) = match receiver.recv().await {
                    Ok((_, data)) if data.is_empty() => return None,
                    Ok(event) => event,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        ("resync", "{}".to_string())
                    }
                    Err(_) => return None,
                };
                let Ok(value) = serde_json::from_str::<serde_json::Value>(&data) else {
                    continue;
                };
                let to = recipients(&host, &tokens, name, &value);
                if to.is_empty() {
                    continue;
                }
                let event = Event::default()
                    .event(name)
                    .data(serde_json::json!({ "to": to, "data": value }).to_string());
                return Some((Ok(event), (host, receiver, tokens, false, window)));
            }
        },
    );
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
/// Which of the extensions holding `tokens` may see a Host event
/// (docs/extensions/ui-model.md, "Event stream").
fn recipients(host: &Host, tokens: &[String], name: &str, data: &serde_json::Value) -> Vec<String> {
    let Ok(credentials) = host.credentials.lock() else {
        return Vec::new();
    };
    let Ok(extensions) = host.extensions.lock() else {
        return Vec::new();
    };
    let states = extensions.list("");
    let mut to: Vec<String> = Vec::new();
    for token in tokens {
        let Some((id, generation)) = credentials.resolve(token) else {
            continue;
        };
        if extensions.generation(&id) != Some(generation) || to.contains(&id) {
            continue;
        }
        let Some(state) = states.iter().find(|s| s.manifest.id == id) else {
            continue;
        };
        let manifest = &state.manifest;
        let has = |capability| manifest.capabilities.contains(&capability);
        let allowed = match name {
            "mail" => has(Capability::MessagesList),
            "sync" | "ready" | "resync" => has(Capability::AccountRead),
            "approval" => has(Capability::ApprovalsNotify),
            "navigate" => data["target"]
                .as_str()
                .is_some_and(|target| manifest.navigation.iter().any(|n| n == target)),
            "extension.settings" => data["id"] == id.as_str(),
            "bus.request" => data["bus"] == id.as_str(),
            "bus.event" => {
                let topic = format!(
                    "{}/{}",
                    data["bus"].as_str().unwrap_or_default(),
                    data["name"].as_str().unwrap_or_default()
                );
                data["bus"] == id.as_str()
                    || (has(Capability::ExtensionsBus)
                        && manifest
                            .bus
                            .as_ref()
                            .is_some_and(|b| b.subscribe.contains(&topic)))
            }
            _ => false,
        };
        if allowed {
            to.push(id);
        }
    }
    to
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
/// Service operations that are not one capability's mail call: storage, labels and the AI
/// models. Identity and account come from the invocation.
fn service_extra(
    host: &Host,
    extension: &str,
    account: &str,
    epoch: u64,
    request: &serde_json::Value,
) -> Option<Result<serde_json::Value, String>> {
    let operation = request["operation"].as_str()?;
    let allowed = |capability| {
        host.extensions
            .lock()
            .is_ok_and(|e| e.allows_generation(extension, account, capability, epoch))
    };
    let current = || {
        host.extensions
            .lock()
            .is_ok_and(|e| e.generation(extension) == Some(epoch))
    };
    Some(match operation {
        "storage.get" if current() => host
            .extensions
            .lock()
            .map_err(|_| "Extension policy unavailable".to_string())
            .map(|e| e.stored(extension, request["key"].as_str().unwrap_or_default())),
        "storage.set" if current() => host
            .extensions
            .lock()
            .map_err(|_| "Extension policy unavailable".to_string())
            .and_then(|mut e| {
                e.store(
                    extension,
                    request["key"].as_str().unwrap_or_default(),
                    request["value"].clone(),
                )
            })
            .map(|()| serde_json::Value::Null),
        "labels.list" if allowed(Capability::MessagesList) => host
            .accounts
            .lock()
            .map_err(|_| "Account service unavailable".to_string())
            .and_then(|accounts| {
                accounts
                    .store(account)
                    .map_err(|_| "Account unavailable".into())
            })
            .and_then(|handle| {
                let store = handle.try_lock().map_err(|_| "Account busy; try again")?;
                serde_json::to_value(store.labels().map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())
            }),
        "ai.settings" if allowed(Capability::AiDecide) => {
            serde_json::to_value(host.ai.public_settings()).map_err(|e| e.to_string())
        }
        "ai.decide" if allowed(Capability::AiDecide) => {
            let state = request["state"].as_str().unwrap_or_default().to_owned();
            let questions: Vec<String> = request["questions"]
                .as_array()
                .map(|list| {
                    list.iter()
                        .filter_map(|q| q.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default();
            if questions.is_empty() || questions.len() > 32 || state.len() > 64 * 1024 {
                return Some(Err("Invalid decision request".into()));
            }
            if !host.ai.take_decision_budget() {
                return Some(Err("The decision model's hourly budget is used up".into()));
            }
            tokio::runtime::Handle::current()
                .block_on(ai::decide(host, state, &questions))
                .map_err(|Failure(_, error)| error)
                .and_then(|answers| serde_json::to_value(answers).map_err(|e| e.to_string()))
        }
        "storage.get" | "storage.set" | "labels.list" | "ai.settings" | "ai.decide" => {
            Err("Capability not granted".into())
        }
        _ => return None,
    })
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
    if request.get("operation").and_then(|v| v.as_str()) == Some("settings.read") {
        if request.as_object().is_none_or(|o| o.len() != 1) {
            return Err("Invalid settings request".into());
        }
        return host
            .extensions
            .lock()
            .map_err(|_| "Extension policy unavailable")?
            .settings_guest(extension, epoch);
    }
    if matches!(
        request["operation"].as_str(),
        Some("bus.request" | "bus.emit")
    ) {
        return bus::service(host, extension, account, epoch, request);
    }
    if let Some(result) = service_extra(host, extension, account, epoch, &request) {
        return result;
    }
    let request: ServiceRequest =
        serde_json::from_value(request).map_err(|_| "Invalid service request")?;
    // Identity and account come from the Host invocation, never from guest input.
    let extensions = host
        .extensions
        .lock()
        .map_err(|_| "Extension policy unavailable")?;
    if !extensions.allows_generation(extension, account, request.operation, epoch) {
        return Err("Capability not granted".into());
    }
    if request.operation == Capability::MessagesSend
        && request
            .draft
            .as_ref()
            .is_some_and(|draft| draft.forward_of.is_some())
        && !extensions.allows_generation(extension, account, Capability::MessagesRead, epoch)
    {
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
        MessagesOrganize
        | AppNavigate
        | SettingsAppearance
        | SettingsNotifications
        | SettingsExtensions
        | ExtensionsBus
        | UiFrame
        | UiFrameRemote
        | UiCommands
        | UiAppActions
        | UiToasts
        | UiContribute
        | UiCanvas
        | ApprovalsNotify
        | ReleasesRead
        | AiText
        | AiDecide
        | AiConfigure => {
            return Err("Capability not granted".into());
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
/// Read message content without holding the store lock: only taking the detached reader
/// locks it, so a refresh in progress never delays opening a message. The demo account
/// has no reader and answers from the store directly.
async fn with_reader<T: Send + 'static>(
    host: Host,
    account: String,
    demo: impl FnOnce(&mut MailStore) -> Result<T, Error> + Send + 'static,
    action: impl FnOnce(&colecto_core::MailReader) -> Result<T, Error> + Send + 'static,
) -> Result<T, Failure> {
    tokio::task::spawn_blocking(move || {
        let handle = host
            .accounts
            .lock()
            .map_err(|_| forbidden())?
            .store(&account)
            .map_err(|_| forbidden())?;
        let unavailable = || {
            Failure(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Mail store unavailable".into(),
            )
        };
        let reader = handle.lock().map_err(|_| unavailable())?.reader();
        match reader {
            Some(reader) => action(&reader).map_err(Failure::from),
            None => demo(&mut *handle.lock().map_err(|_| unavailable())?).map_err(Failure::from),
        }
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
struct ContentQuery {
    #[serde(default)]
    cached: bool,
}
#[derive(serde::Deserialize, Default)]
struct MessageQuery {
    #[serde(default = "inbox_folder")]
    folder: String,
    #[serde(default)]
    q: String,
    /// Messages to skip from the newest, to load an older page (API 2).
    #[serde(default)]
    offset: usize,
}
impl MessageQuery {
    /// Other folders and older pages arrived in API 2; extensions on API 1 keep
    /// getting the error they always got for them.
    fn check_api_version(&self, host: &Host, principal: &Principal) -> Result<(), Failure> {
        if matches!(self.folder.as_str(), "inbox" | "sent") && self.offset == 0 {
            return Ok(());
        }
        let Principal::Extension(token) = principal else {
            return Ok(());
        };
        let (id, _) = caller_extension(host, token)?;
        if host
            .extensions
            .lock()
            .map_err(|_| forbidden())?
            .api_version(&id)
            .is_some_and(|version| version < 2)
        {
            return Err(Failure(
                StatusCode::BAD_REQUEST,
                "Other folders and offset need api_version = 2 or later".into(),
            ));
        }
        Ok(())
    }
}
fn inbox_folder() -> String {
    "inbox".into()
}
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct OrganizeInput {
    ids: Vec<String>,
    seen: Option<bool>,
    flagged: Option<bool>,
    folder: Option<String>,
}
async fn organize_messages(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    axum::extract::Path(action): axum::extract::Path<String>,
    input: Result<Json<OrganizeInput>, JsonRejection>,
) -> Result<Json<serde_json::Value>, Failure> {
    let account = grant(&host, &principal, &headers, Capability::MessagesOrganize)?;
    let Json(input) = input.map_err(|error| Failure(StatusCode::BAD_REQUEST, error.body_text()))?;
    if input.ids.is_empty() || input.ids.len() > 500 {
        return Err(Failure(
            StatusCode::BAD_REQUEST,
            "Supply between 1 and 500 message ids".into(),
        ));
    }
    with_store(host.clone(), account.clone(), move |store| {
        match action.as_str() {
            "flags" => store.set_flags(&input.ids, input.seen, input.flagged),
            "move" => store.move_to(
                &input.ids,
                input
                    .folder
                    .as_deref()
                    .ok_or_else(|| colecto_core::Error::Invalid("Folder required".into()))?,
            ),
            "archive" => store.archive(&input.ids),
            "delete" => store.delete(&input.ids),
            _ => Err(colecto_core::Error::Invalid("Unknown operation".into())),
        }
    })
    .await?;
    let _ = host
        .events
        .send(("mail", serde_json::json!({"account":account}).to_string()));
    Ok(Json(serde_json::json!({"ok":true})))
}

async fn folders(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
) -> Result<Json<Vec<colecto_extension_api::Folder>>, Failure> {
    let account = grant(&host, &principal, &headers, Capability::MessagesList)?;
    with_store(host, account, |s| s.folders()).await.map(Json)
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
    let labels = with_store(host, account.clone(), move |s| {
        s.update_label(&input.name, input.message_id.as_deref(), input.remove)
    })
    .await?;
    Ok(Json(labels))
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
    query.check_api_version(&host, &principal)?;
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
    query.check_api_version(&host, &principal)?;
    host.accounts
        .lock()
        .map_err(|_| forbidden())?
        .store(&account)
        .map_err(|_| forbidden())?;
    let (worker, id) = (host.clone(), account.clone());
    let mut messages = tokio::task::spawn_blocking(move || {
        let messages = sync::refresh(
            &worker,
            &id,
            &query.folder,
            &query.q,
            query.offset,
            sync::Origin::Owner,
        )?
        .unwrap_or_default();
        if query.folder == "inbox" && query.q.trim().is_empty() && query.offset == 0 {
            sync::start_prefetch(&worker, &id);
        }
        Ok::<_, Error>(messages)
    })
    .await
    .map_err(|_| {
        Failure(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Mail service unavailable".into(),
        )
    })??;
    redact_list_bodies(&host, &principal, &account, &mut messages)?;
    Ok(Json(messages))
}
/// Sync mode and progress for every account, also streamed as `sync` events.
async fn sync_status(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
) -> Result<Json<Vec<sync::AccountSync>>, Failure> {
    if matches!(principal, Principal::Extension(_)) {
        grant(&host, &principal, &headers, Capability::AccountRead)?;
    } else {
        host_grant(&headers)?;
    }
    Ok(Json(
        tokio::task::spawn_blocking(move || sync::statuses(&host))
            .await
            .unwrap_or_default(),
    ))
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
    Query(query): Query<ContentQuery>,
) -> Result<Json<MessageContent>, Failure> {
    let account = grant(&host, &principal, &headers, Capability::MessagesRead)?;
    let demo = id.clone();
    with_reader(
        host,
        account,
        move |store| store.content(&demo),
        move |reader| {
            // Clients warm their own caches with `cached=true`; that never reaches the provider.
            if query.cached && !reader.is_cached(&id) {
                return Err(Error::Missing);
            }
            reader.content(&id)
        },
    )
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
    let demo = id.clone();
    let (info, data) = with_reader(
        host,
        account,
        move |store| store.attachment(&demo, index),
        move |reader| reader.attachment(&id, index),
    )
    .await?;
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
#[derive(serde::Deserialize)]
struct DiscoverInput {
    address: String,
}
/// Looks up server settings for an address; `null` when nothing usable was found.
async fn discover_account(
    headers: HeaderMap,
    input: Result<Json<DiscoverInput>, JsonRejection>,
) -> Result<Json<Option<discovery::Discovery>>, Failure> {
    host_grant(&headers)?;
    let Json(input) =
        input.map_err(|_| Failure(StatusCode::BAD_REQUEST, "Invalid email address".into()))?;
    if input.address.len() > 320 {
        return Err(Failure(
            StatusCode::BAD_REQUEST,
            "Invalid email address".into(),
        ));
    }
    let found = tokio::time::timeout(discovery::DEADLINE, discovery::discover(&input.address))
        .await
        .ok()
        .flatten();
    Ok(Json(found))
}
async fn remove_account(
    State(host): State<Host>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<bool>, Failure> {
    host_grant(&headers)?;
    tokio::task::spawn_blocking(move || {
        host.accounts.lock().map_err(|_| forbidden())?.remove(&id)?;
        host.ai.remove_account(&id);
        host.preferences
            .lock()
            .map_err(|_| forbidden())?
            .clear_account(&id)
            .map_err(|e| Failure(StatusCode::BAD_REQUEST, e))?;
        Ok(Json(true))
    })
    .await
    .map_err(|_| forbidden())?
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
) -> Result<Json<preferences::PreferencesView>, Failure> {
    host_grant(&headers)?;
    Ok(Json(
        host.preferences.lock().map_err(|_| forbidden())?.view(),
    ))
}
async fn update_preferences(
    State(host): State<Host>,
    headers: HeaderMap,
    input: Result<Json<preferences::PreferencesInput>, JsonRejection>,
) -> Result<Json<preferences::PreferencesView>, Failure> {
    host_grant(&headers)?;
    let Json(input) =
        input.map_err(|_| Failure(StatusCode::BAD_REQUEST, "Invalid preferences".into()))?;
    tokio::task::spawn_blocking(move || {
        let mut store = host.preferences.lock().map_err(|_| forbidden())?;
        let before = store.get().mcp;
        let saved = store
            .update(input)
            .map_err(|e| Failure(StatusCode::BAD_REQUEST, e))?;
        // The desktop app opens or closes its API port with MCP.
        if saved.mcp != before {
            let _ = host
                .events
                .send(("mcp", serde_json::json!({"on": saved.mcp}).to_string()));
        }
        Ok(store.view())
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
        let store = store.try_lock().map_err(|_| {
            Failure(
                StatusCode::TOO_MANY_REQUESTS,
                "Mail service busy; try again".into(),
            )
        })?;
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

async fn get_extension_settings(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, Failure> {
    own_or_owner(&host, &principal, &headers, &id)?;
    host.extensions
        .lock()
        .map_err(|_| forbidden())?
        .settings(&id)
        .map(Json)
        .map_err(|e| Failure(StatusCode::BAD_REQUEST, e))
}
async fn put_extension_settings(
    State(host): State<Host>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Path(id): Path<String>,
    input: Result<Json<serde_json::Map<String, serde_json::Value>>, JsonRejection>,
) -> Result<Json<serde_json::Value>, Failure> {
    own_or_owner(&host, &principal, &headers, &id)?;
    let Json(values) =
        input.map_err(|_| Failure(StatusCode::BAD_REQUEST, "Invalid settings".into()))?;
    let result = host
        .extensions
        .lock()
        .map_err(|_| forbidden())?
        .update_settings(&id, values)
        .map_err(|e| Failure(StatusCode::BAD_REQUEST, e))?;
    let _ = host.events.send((
        "extension.settings",
        serde_json::json!({"id":id}).to_string(),
    ));
    Ok(Json(result))
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct ResetInput {
    keys: Vec<String>,
}
/// Return an extension's settings to their defaults. Owner only, from Settings.
async fn reset_extension_settings(
    State(host): State<Host>,
    headers: HeaderMap,
    Path(id): Path<String>,
    input: Result<Json<ResetInput>, JsonRejection>,
) -> Result<Json<serde_json::Value>, Failure> {
    host_grant(&headers)?;
    let Json(input) =
        input.map_err(|_| Failure(StatusCode::BAD_REQUEST, "Invalid settings".into()))?;
    let result = host
        .extensions
        .lock()
        .map_err(|_| forbidden())?
        .reset_settings(&id, &input.keys)
        .map_err(|e| Failure(StatusCode::BAD_REQUEST, e))?;
    let _ = host.events.send((
        "extension.settings",
        serde_json::json!({"id":id}).to_string(),
    ));
    Ok(Json(result))
}

#[cfg(test)]
mod extension_service_tests {
    use super::*;
    #[tokio::test]
    async fn guest_service_rejects_identity_injection_and_checks_current_grants() {
        let dir = tempfile::tempdir().unwrap();
        let accounts = Accounts::open(
            MailStore::open(dir.path()).unwrap(),
            Arc::new(UnavailableSecrets),
        )
        .unwrap();
        let database = accounts.database();
        let mut extensions = extensions::Extensions::open(database.clone()).unwrap();
        extensions
            .install(&extensions::tests::package(
                &extensions::tests::EXAMPLE.replace(
                    "capabilities = [\"account.read\"]",
                    "capabilities = [\"account.read\", \"messages.send\"]",
                ),
                Some(wat::parse_str("(module (func (export \"run\")))").unwrap()),
            ))
            .unwrap();
        extensions.update("hello-pane", true).unwrap();
        let host = Host {
            events: tokio::sync::broadcast::channel(1).0,
            bus_replies: Default::default(),
            keys: Arc::new(Mutex::new(ApiKeys::open(database.clone()).unwrap())),
            ai: Arc::new(ai::Ai::open(database.clone()).unwrap()),
            preferences: Arc::new(Mutex::new(
                preferences::PreferenceStore::open(database.clone()).unwrap(),
            )),
            approvals: Arc::new(Mutex::new(
                approvals::Approvals::open(database, api_keys::now()).unwrap(),
            )),
            status_polls: Arc::default(),
            accounts: Arc::new(Mutex::new(accounts)),
            extensions: Arc::new(Mutex::new(extensions)),
            authority: "127.0.0.1:4173".into(),
            origin: "http://127.0.0.1:4173".into(),
            execution: Arc::new(tokio::sync::Semaphore::new(2)),
            sync: Arc::default(),
            credentials: Arc::default(),
            windows: Arc::default(),
        };
        let epoch = host
            .extensions
            .lock()
            .unwrap()
            .service("hello-pane")
            .unwrap()
            .0;
        assert!(host.extensions.lock().unwrap().allows_generation(
            "hello-pane",
            "demo",
            Capability::MessagesSend,
            epoch
        ));
        assert!(!host.extensions.lock().unwrap().allows_generation(
            "hello-pane",
            "demo",
            Capability::MessagesRead,
            epoch
        ));
        let forward = serde_json::json!({"operation":"messages.send", "draft":{"recipient":"a@example.test", "subject":"Forward", "body":"Hi", "forward_of":"welcome"}});
        assert_eq!(
            extension_service(&host, "hello-pane", "demo", epoch, forward).unwrap_err(),
            "Capability not granted"
        );
        assert_eq!(
            extension_service(
                &host,
                "hello-pane",
                "demo",
                epoch,
                serde_json::json!({"operation":"settings.read"})
            )
            .unwrap(),
            serde_json::json!({"greeting": "Hello"})
        );
        assert!(
            extension_service(
                &host,
                "hello-pane",
                "demo",
                epoch + 1,
                serde_json::json!({"operation":"settings.read"})
            )
            .is_err()
        );
        let valid = serde_json::json!({"operation":"account.read"});
        assert_eq!(
            extension_service(&host, "hello-pane", "demo", epoch, valid.clone()).unwrap(),
            false
        );
        for invalid in [
            serde_json::json!({"operation":"account.read", "account":"other"}),
            serde_json::json!({"operation":"account.read", "extension":"host"}),
            serde_json::json!({"operation":"accounts.manage"}),
            serde_json::json!({"operation":"messages.read", "id":"1"}),
            serde_json::json!({"operation":"messages.label", "label":{"name":"Work", "remove":false}}),
        ] {
            assert!(extension_service(&host, "hello-pane", "demo", epoch, invalid).is_err());
        }
        assert!(extension_service(&host, "hello-pane", "other", epoch, valid.clone()).is_err());
        host.extensions
            .lock()
            .unwrap()
            .update("hello-pane", false)
            .unwrap();
        assert!(extension_service(&host, "hello-pane", "demo", epoch, valid).is_err());
        let account = {
            let mut accounts = host.accounts.lock().unwrap();
            accounts.protect("a strong test password").unwrap();
            accounts.upsert(serde_json::from_value(serde_json::json!({
                "address":"from@example.test", "protocol":"jmap",
                "session_url":"https://example.test/jmap", "username":"from", "secret":"secret"
            })).unwrap()).unwrap().id
        };
        let pending = host
            .approvals
            .lock()
            .unwrap()
            .create(
                "key",
                &account,
                "from@example.test",
                serde_json::from_value(
                    serde_json::json!({"recipient":"a@example.test", "subject":"Hi", "body":"Hi"}),
                )
                .unwrap(),
                api_keys::now(),
            )
            .unwrap();
        let store = host.accounts.lock().unwrap().store(&account).unwrap();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let busy = std::thread::spawn(move || {
            let _guard = store.lock().unwrap();
            ready_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        });
        ready_rx.recv().unwrap();
        let mut headers = HeaderMap::new();
        headers.insert("x-colecto-extension", "host".parse().unwrap());
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            approve_send(
                State(host.clone()),
                axum::Extension(Principal::Owner),
                headers,
                Path(pending.id.clone()),
            ),
        )
        .await
        .unwrap();
        assert!(matches!(
            result,
            Err(Failure(StatusCode::TOO_MANY_REQUESTS, _))
        ));
        assert!(host.accounts.try_lock().is_ok());
        assert_eq!(
            host.approvals
                .lock()
                .unwrap()
                .get(&pending.id, api_keys::now())
                .unwrap()
                .status,
            approvals::Status::Pending
        );
        release_tx.send(()).unwrap();
        busy.join().unwrap();
    }
}
