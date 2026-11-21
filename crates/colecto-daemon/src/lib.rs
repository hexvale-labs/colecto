//! HTTP + SSE + MCP daemon adapter over `colecto_service::MailService`.
pub mod mcp;
pub mod runtime;

pub use colecto_service::{
    MailService, VERSION, ai, api_keys, approvals, commit, discovery, error, events, logging,
    notifications, preferences, releases, secrets, settings, sync,
};

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, Request, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{
        IntoResponse, Response, Sse,
        sse::{Event, KeepAlive},
    },
    routing::{delete, get, post},
};
use colecto_core::{
    AccountInfo, AccountInput, Accounts, ApiError, Draft, DraftInput, ErrorCategory, Folder,
    LabelInput, MailSnapshot, MailStore, Message, MessageContent, SecurityInfo, SecurityInput,
    threading::ThreadMessage,
};
use colecto_service::{
    ContentOptions, DeliveryOutcome, MessageQuery, NavigateResult, OrganizeInput, ReleaseStatus,
    SendStatus,
    api_keys::Principal,
    error::{ErrorKind, ServiceError},
    events::ServiceEvent,
    secrets::UnavailableSecrets,
};
use serde::Deserialize;
use std::{collections::BTreeMap, convert::Infallible, path::PathBuf, sync::Arc};
use subtle::ConstantTimeEq;
use tower_http::services::ServeDir;
use zeroize::Zeroizing;

/// Looks up a request path such as `/index.html`, returning its bytes and MIME type.
pub type AssetLookup = Arc<dyn Fn(&str) -> Option<(Vec<u8>, String)> + Send + Sync>;

/// Where optional static web assets come from.
#[derive(Clone)]
pub enum Assets {
    Directory(PathBuf),
    Embedded(AssetLookup),
    None,
}

impl From<PathBuf> for Assets {
    fn from(directory: PathBuf) -> Self {
        Self::Directory(directory)
    }
}

#[derive(Clone)]
pub struct AccessPolicy {
    authority: String,
    local: bool,
    origin: String,
    token: Arc<Zeroizing<String>>,
}

impl AccessPolicy {
    pub fn local(authority: String, token: String) -> Result<Self, String> {
        if !valid_token(&token) {
            return Err(
                "Access token must contain 64–512 ASCII letters, digits, hyphens or underscores"
                    .into(),
            );
        }
        Ok(Self {
            local: true,
            origin: format!("http://{authority}"),
            authority,
            token: Arc::new(Zeroizing::new(token)),
        })
    }

    pub fn local_ephemeral(authority: String) -> Result<Self, String> {
        let token = colecto_service::api_keys::random()?;
        Self::local(authority, token)
    }

    pub fn remote(origin: &str, token: String) -> Result<Self, String> {
        let origin = parse_origin(origin, false)
            .ok_or("Public origin must be an HTTPS origin without path, credentials, or query")?;
        if !valid_token(&token) {
            return Err(
                "Access token must contain 64–512 ASCII letters, digits, hyphens or underscores"
                    .into(),
            );
        }
        Ok(Self {
            local: false,
            authority: origin.strip_prefix("https://").unwrap().into(),
            origin,
            token: Arc::new(Zeroizing::new(token)),
        })
    }

    pub fn authority(&self) -> &str {
        &self.authority
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }

    pub fn token(&self) -> &str {
        self.token.as_str()
    }

    pub fn is_local(&self) -> bool {
        self.local
    }

    pub fn allows_host(&self, host: &str) -> bool {
        if host == self.authority {
            return true;
        }
        self.local_localhost_authority()
            .is_some_and(|alias| host == alias)
    }

    pub fn allows_origin(&self, origin: &str) -> bool {
        if origin == self.origin {
            return true;
        }
        self.local_localhost_authority()
            .is_some_and(|alias| origin.strip_prefix("http://") == Some(alias.as_str()))
    }

    fn local_localhost_authority(&self) -> Option<String> {
        if !self.local {
            return None;
        }
        let port = self
            .authority
            .strip_prefix("127.0.0.1:")
            .or_else(|| self.authority.strip_prefix("[::1]:"))?;
        Some(format!("localhost:{port}"))
    }
}

/// Owner tokens are 64–512 ASCII letters, digits, hyphens or underscores.
pub(crate) fn valid_token(token: &str) -> bool {
    (64..=512).contains(&token.len())
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
}

/// Validate and normalize an origin-only URL (`https://...`, plus loopback `http://` when allowed).
pub(crate) fn parse_origin(input: &str, allow_loopback_http: bool) -> Option<String> {
    let url = reqwest::Url::parse(input).ok()?;
    let loopback = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
    let scheme_ok =
        url.scheme() == "https" || (allow_loopback_http && url.scheme() == "http" && loopback);
    if !scheme_ok
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.path() != "/"
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return None;
    }
    Some(url.origin().ascii_serialization())
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) service: MailService,
    pub(crate) policy: AccessPolicy,
}

#[derive(Clone)]
pub struct Daemon {
    service: MailService,
    policy: AccessPolicy,
    router: Router,
}

impl Daemon {
    pub fn new(
        service: MailService,
        policy: AccessPolicy,
        assets: impl Into<Assets>,
    ) -> Result<Self, String> {
        let assets = assets.into();
        let data_dir = service.data_directory();
        if let Assets::Directory(directory) = &assets {
            match std::fs::canonicalize(directory) {
                Ok(root) if data_dir.starts_with(&root) => {
                    return Err("Static assets must not contain the private data directory".into());
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err("Cannot validate static assets directory".into()),
            }
        }
        let state = AppState {
            service: service.clone(),
            policy: policy.clone(),
        };
        let router = build_router(state, assets);
        Ok(Self {
            service,
            policy,
            router,
        })
    }

    pub fn router(&self) -> Router {
        self.router.clone()
    }

    pub fn service(&self) -> &MailService {
        &self.service
    }

    pub fn policy(&self) -> &AccessPolicy {
        &self.policy
    }
}

/// Convenience constructor for local tests.
pub fn app(store: MailStore, authority: String, token: String, assets: PathBuf) -> Router {
    let accounts = Accounts::open(store, Arc::new(UnavailableSecrets)).expect("open accounts");
    app_with_accounts(accounts, authority, token, assets)
}

pub fn app_with_accounts(
    accounts: Accounts,
    authority: String,
    token: String,
    assets: PathBuf,
) -> Router {
    let service = MailService::new(accounts).expect("Cannot initialize mail service");
    let policy = AccessPolicy::local(authority, token).expect("Invalid local access policy");
    Daemon::new(service, policy, assets)
        .expect("Cannot initialize daemon")
        .router()
}

pub(crate) struct Failure {
    pub(crate) status: StatusCode,
    pub(crate) message: String,
    pub(crate) category: ErrorCategory,
}

impl Failure {
    pub(crate) fn forbidden() -> Self {
        Self::from(ServiceError::forbidden())
    }

    pub(crate) fn bad_request(message: impl Into<String>) -> Self {
        Self::from(ServiceError::bad_request(message))
    }

    pub(crate) fn not_found(message: impl Into<String>) -> Self {
        Self::from(ServiceError::not_found(message))
    }
}

impl From<ServiceError> for Failure {
    fn from(error: ServiceError) -> Self {
        let status = match error.kind {
            ErrorKind::Authentication => StatusCode::UNAUTHORIZED,
            ErrorKind::Forbidden => StatusCode::FORBIDDEN,
            ErrorKind::BadRequest => StatusCode::BAD_REQUEST,
            ErrorKind::NotFound => StatusCode::NOT_FOUND,
            ErrorKind::Conflict => StatusCode::CONFLICT,
            ErrorKind::TooManyRequests => StatusCode::TOO_MANY_REQUESTS,
            ErrorKind::BadGateway => StatusCode::BAD_GATEWAY,
            ErrorKind::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        };
        let category = error.category();
        Self {
            status,
            message: error.message,
            category,
        }
    }
}

impl IntoResponse for Failure {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ApiError {
                error: self.message,
                category: self.category,
            }),
        )
            .into_response()
    }
}

fn build_router(state: AppState, assets: Assets) -> Router {
    let mcp_router = Router::new()
        .nest_service(
            "/mcp",
            mcp::http(state.service.clone(), state.policy.authority().to_owned()),
        )
        .layer(middleware::from_fn_with_state(state.clone(), mcp_enabled));

    let api = Router::new()
        .route("/version", get(version))
        .route("/releases/latest", get(latest_release))
        .route("/events", get(events_stream))
        .route(
            "/preferences",
            get(get_preferences).post(update_preferences),
        )
        .route("/api-keys", get(list_api_keys).post(mint_api_key))
        .route("/api-keys/{id}", delete(revoke_api_key))
        .route("/accounts", get(list_accounts).post(upsert_account))
        .route("/accounts/discover", post(discover_account))
        .route("/accounts/{id}", delete(remove_account))
        .route("/accounts/{id}/disconnect", post(disconnect_account))
        .route("/security", get(security).post(security_action))
        .route("/account", get(account))
        .route("/sync", get(sync_status))
        .route("/folders", get(folders))
        .route("/labels", get(labels).post(update_label))
        .route("/messages", get(messages))
        .route("/messages/cache", get(cached_messages))
        .route("/messages/refresh", post(messages))
        .route("/messages/{id}", get(message).post(organize_messages))
        .route("/messages/{id}/content", get(message_content))
        .route(
            "/messages/{id}/attachments/{index}",
            get(message_attachment),
        )
        .route("/messages/{id}/conversation", get(message_conversation))
        .route("/drafts", get(drafts).post(save_draft))
        .route("/drafts/{id}", get(draft))
        .route("/send", post(send).layer(DefaultBodyLimit::max(36_000_000)))
        .route("/send-requests", get(list_send_requests))
        .route("/send-requests/{id}", get(send_request))
        .route("/send-requests/{id}/approve", post(approve_send))
        .route("/send-requests/{id}/reject", post(reject_send))
        .route("/navigate", post(navigate))
        .route("/ai/settings", get(ai_get_settings).post(ai_put_settings))
        .route("/ai/keys", post(ai_put_key))
        .route("/ai/test", post(ai_test_connection))
        .route("/ai/labels/preview", post(ai_preview))
        .route("/ai/reclassify", post(ai_reclassify))
        .route(
            "/ai/summaries/{id}",
            get(ai_get_summary)
                .post(ai_create_summary)
                .delete(ai_delete_summary),
        )
        .route("/ai/reply", post(ai_draft_reply))
        .merge(mcp_router)
        .fallback(|| async { Failure::not_found("API route not found") });

    let router = Router::new().nest("/api/v1", api);
    let router = match assets {
        Assets::Directory(directory) => router.fallback_service(ServeDir::new(directory)),
        Assets::Embedded(lookup) => router.fallback(move |uri: axum::http::Uri| async move {
            match lookup(uri.path()) {
                Some((body, mime)) => ([(header::CONTENT_TYPE, mime)], body).into_response(),
                None => StatusCode::NOT_FOUND.into_response(),
            }
        }),
        Assets::None => router.fallback(|| async { StatusCode::NOT_FOUND }),
    };

    router
        .layer(DefaultBodyLimit::max(512_000))
        .layer(
            tower_http::trace::TraceLayer::new_for_http().make_span_with(
                |request: &axum::http::Request<_>| {
                    tracing::debug_span!(
                        "request",
                        method = %request.method(),
                        path = request.uri().path()
                    )
                },
            ),
        )
        .layer(middleware::from_fn_with_state(state.clone(), authenticate))
        .layer(middleware::from_fn_with_state(state.clone(), boundary))
        .with_state(state)
}

async fn boundary(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let headers = request.headers();
    let authority_ok = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|host| state.policy.allows_host(host));
    let origin = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok());
    let mutation = !matches!(
        *request.method(),
        axum::http::Method::GET | axum::http::Method::HEAD
    );
    let has_bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|value| value.starts_with("Bearer "));
    let origin_ok = match origin {
        Some(value) => state.policy.allows_origin(value),
        None => !mutation || has_bearer,
    };
    let mut response = if !authority_ok || !origin_ok {
        Failure::forbidden().into_response()
    } else {
        next.run(request).await
    };
    let headers = response.headers_mut();
    headers.entry(header::CONTENT_SECURITY_POLICY).or_insert(
        HeaderValue::from_static(
            "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob: https:; object-src 'none'; base-uri 'none'; frame-ancestors 'none'",
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

async fn mcp_enabled(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if state.service.mcp_enabled() {
        next.run(request).await
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "MCP is turned off. Turn it on in Settings → API keys."
            })),
        )
            .into_response()
    }
}

async fn authenticate(State(state): State<AppState>, mut request: Request, next: Next) -> Response {
    if !request.uri().path().starts_with("/api/") || request.uri().path() == "/api/v1/version" {
        return next.run(request).await;
    }
    let session_request = request.uri().path() == "/api/v1/session";
    let bearer = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "));

    let principal = bearer.and_then(|supplied| {
        if supplied
            .as_bytes()
            .ct_eq(state.policy.token().as_bytes())
            .unwrap_u8()
            == 1
        {
            Some(Principal::Owner)
        } else {
            state.service.authenticate_api_key(supplied).ok()
        }
    });

    let Some(principal) = principal else {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": "Enter the daemon access token to connect"
            })),
        )
            .into_response();
    };

    if matches!(principal, Principal::ApiKey(_)) {
        let path = request.uri().path();
        if !matches!(
            path,
            "/api/v1/account"
                | "/api/v1/messages"
                | "/api/v1/folders"
                | "/api/v1/drafts"
                | "/api/v1/labels"
                | "/api/v1/send"
                | "/api/v1/mcp"
                | "/api/v1/navigate"
        ) && !path.starts_with("/api/v1/messages/")
            && !path.starts_with("/api/v1/drafts/")
            && !(request.method() == axum::http::Method::GET
                && path.starts_with("/api/v1/send-requests/"))
        {
            return Failure::forbidden().into_response();
        }
    }

    if session_request {
        if request.method() != axum::http::Method::POST {
            return StatusCode::METHOD_NOT_ALLOWED.into_response();
        }
        return Json(true).into_response();
    }

    request.extensions_mut().insert(principal);
    next.run(request).await
}

fn account_header(headers: &HeaderMap) -> Result<String, Failure> {
    headers
        .get("x-colecto-account")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(Failure::forbidden)
}

fn optional_account_header(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-colecto-account")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

async fn version() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "name": "colecto",
        "version": VERSION,
        "commit": commit(),
        "api": "v1",
    }))
}

async fn latest_release(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
) -> Result<Json<ReleaseStatus>, Failure> {
    state
        .service
        .latest_release(&principal)
        .await
        .map(Json)
        .map_err(Failure::from)
}

#[derive(Deserialize, Default)]
struct EventsQuery {
    #[serde(default)]
    window: bool,
}

async fn events_stream(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    Query(query): Query<EventsQuery>,
) -> Result<impl IntoResponse, Failure> {
    if !matches!(principal, Principal::Owner) {
        return Err(Failure::forbidden());
    }
    let receiver = state.service.subscribe();
    let stopped = state.service.closing_receiver();
    let window = query.window.then(|| state.service.open_window());
    let service = state.service.clone();
    let stream = futures_util::stream::unfold(
        (receiver, stopped, true, service, window),
        |(mut receiver, mut stopped, first, service, window)| async move {
            if *stopped.borrow() {
                return None;
            }
            if first {
                return Some((
                    Ok::<_, Infallible>(Event::default().event("ready").data("{}")),
                    (receiver, stopped, false, service, window),
                ));
            }
            let event = tokio::select! {
                _ = stopped.changed() => return None,
                event = receiver.recv() => event,
            };
            match event {
                Ok(ServiceEvent::Shutdown) => None,
                Ok(event) => {
                    let name = event.event_name();
                    let data = event.payload_json(Some(service.open_windows()));
                    Some((
                        Ok(Event::default().event(name).data(data)),
                        (receiver, stopped, false, service, window),
                    ))
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => Some((
                    Ok(Event::default().event("resync").data("{}")),
                    (receiver, stopped, false, service, window),
                )),
                Err(_) => None,
            }
        },
    );
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

async fn get_preferences(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
) -> Result<Json<preferences::PreferencesView>, Failure> {
    state
        .service
        .get_preferences(&principal)
        .map(Json)
        .map_err(Failure::from)
}

async fn update_preferences(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    input: Result<Json<preferences::PreferencesInput>, JsonRejection>,
) -> Result<Json<preferences::PreferencesView>, Failure> {
    let Json(input) = input.map_err(|_| Failure::bad_request("Invalid preferences"))?;
    state
        .service
        .update_preferences(&principal, input)
        .await
        .map(Json)
        .map_err(Failure::from)
}

async fn list_api_keys(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
) -> Result<Json<Vec<api_keys::KeyMetadata>>, Failure> {
    state
        .service
        .list_api_keys(&principal)
        .map(Json)
        .map_err(Failure::from)
}

async fn mint_api_key(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    input: Result<Json<api_keys::KeyInput>, JsonRejection>,
) -> Result<Json<serde_json::Value>, Failure> {
    let Json(input) = input.map_err(|_| Failure::bad_request("Invalid API key settings"))?;
    let (key, secret) = state.service.mint_api_key(&principal, input)?;
    Ok(Json(serde_json::json!({ "key": key, "secret": secret })))
}

async fn revoke_api_key(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    Path(id): Path<String>,
) -> Result<Json<bool>, Failure> {
    state
        .service
        .revoke_api_key(&principal, &id)
        .map(Json)
        .map_err(Failure::from)
}

async fn list_accounts(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
) -> Result<Json<Vec<AccountInfo>>, Failure> {
    state
        .service
        .list_accounts(&principal)
        .await
        .map(Json)
        .map_err(Failure::from)
}

async fn upsert_account(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    input: Result<Json<AccountInput>, JsonRejection>,
) -> Result<Json<AccountInfo>, Failure> {
    let Json(input) = input.map_err(|_| Failure::bad_request("Invalid account settings"))?;
    state
        .service
        .upsert_account(&principal, input)
        .await
        .map(Json)
        .map_err(Failure::from)
}

#[derive(Deserialize)]
struct DiscoverInput {
    address: String,
}

async fn discover_account(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    input: Result<Json<DiscoverInput>, JsonRejection>,
) -> Result<Json<Option<discovery::Discovery>>, Failure> {
    let Json(input) = input.map_err(|_| Failure::bad_request("Invalid email address"))?;
    state
        .service
        .discover_account(&principal, &input.address)
        .await
        .map(Json)
        .map_err(Failure::from)
}

async fn remove_account(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    Path(id): Path<String>,
) -> Result<Json<bool>, Failure> {
    state
        .service
        .remove_account(&principal, &id)
        .await
        .map(Json)
        .map_err(Failure::from)
}

async fn disconnect_account(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    Path(id): Path<String>,
) -> Result<Json<bool>, Failure> {
    state
        .service
        .disconnect_account(&principal, &id)
        .await
        .map(Json)
        .map_err(Failure::from)
}

async fn security(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
) -> Result<Json<SecurityInfo>, Failure> {
    state
        .service
        .security(&principal)
        .await
        .map(Json)
        .map_err(Failure::from)
}

async fn security_action(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    input: Result<Json<SecurityInput>, JsonRejection>,
) -> Result<Json<SecurityInfo>, Failure> {
    let Json(input) = input.map_err(|_| Failure::bad_request("Invalid security request"))?;
    state
        .service
        .security_action(&principal, input)
        .await
        .map(Json)
        .map_err(Failure::from)
}

async fn account(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
) -> Result<Json<bool>, Failure> {
    let account = account_header(&headers)?;
    state
        .service
        .account_connected(&principal, &account)
        .await
        .map(Json)
        .map_err(Failure::from)
}

async fn sync_status(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
) -> Result<Json<Vec<sync::AccountSync>>, Failure> {
    state
        .service
        .sync_status(&principal)
        .await
        .map(Json)
        .map_err(Failure::from)
}

async fn folders(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
) -> Result<Json<Vec<Folder>>, Failure> {
    let account = account_header(&headers)?;
    state
        .service
        .folders(&principal, &account)
        .await
        .map(Json)
        .map_err(Failure::from)
}

async fn labels(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
) -> Result<Json<BTreeMap<String, Vec<String>>>, Failure> {
    let account = account_header(&headers)?;
    state
        .service
        .labels(&principal, &account)
        .await
        .map(Json)
        .map_err(Failure::from)
}

async fn update_label(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    input: Result<Json<LabelInput>, JsonRejection>,
) -> Result<Json<BTreeMap<String, Vec<String>>>, Failure> {
    let account = account_header(&headers)?;
    let Json(input) = input.map_err(|e| Failure::bad_request(e.body_text()))?;
    state
        .service
        .update_label(&principal, &account, input)
        .await
        .map(Json)
        .map_err(Failure::from)
}

async fn cached_messages(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Query(query): Query<MessageQuery>,
) -> Result<Json<MailSnapshot>, Failure> {
    let account = account_header(&headers)?;
    state
        .service
        .cached_messages(&principal, &account, query)
        .await
        .map(Json)
        .map_err(Failure::from)
}

async fn messages(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Query(query): Query<MessageQuery>,
) -> Result<Json<Vec<Message>>, Failure> {
    let account = account_header(&headers)?;
    state
        .service
        .messages(&principal, &account, query)
        .await
        .map(Json)
        .map_err(Failure::from)
}

async fn organize_messages(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Path(action): Path<String>,
    input: Result<Json<OrganizeInput>, JsonRejection>,
) -> Result<Json<serde_json::Value>, Failure> {
    let account = account_header(&headers)?;
    let Json(input) = input.map_err(|e| Failure::bad_request(e.body_text()))?;
    state
        .service
        .organize_messages(&principal, &account, &action, input)
        .await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn message(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Message>, Failure> {
    let account = account_header(&headers)?;
    state
        .service
        .message(&principal, &account, &id)
        .await
        .map(Json)
        .map_err(Failure::from)
}

async fn message_content(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(options): Query<ContentOptions>,
) -> Result<Json<MessageContent>, Failure> {
    let account = account_header(&headers)?;
    state
        .service
        .message_content(&principal, &account, &id, options)
        .await
        .map(Json)
        .map_err(Failure::from)
}

async fn message_attachment(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Path((id, index)): Path<(String, usize)>,
) -> Result<Response, Failure> {
    let account = account_header(&headers)?;
    let (info, data) = state
        .service
        .message_attachment(&principal, &account, &id, index)
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

async fn message_conversation(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Vec<ThreadMessage>>, Failure> {
    let account = account_header(&headers)?;
    state
        .service
        .conversation(&principal, &account, &id)
        .await
        .map(Json)
        .map_err(Failure::from)
}

async fn drafts(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
) -> Result<Json<Vec<Draft>>, Failure> {
    let account = account_header(&headers)?;
    state
        .service
        .drafts(&principal, &account)
        .await
        .map(Json)
        .map_err(Failure::from)
}

async fn draft(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Draft>, Failure> {
    let account = account_header(&headers)?;
    state
        .service
        .draft(&principal, &account, &id)
        .await
        .map(Json)
        .map_err(Failure::from)
}

async fn save_draft(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    input: Result<Json<DraftInput>, JsonRejection>,
) -> Result<Json<Draft>, Failure> {
    let account = account_header(&headers)?;
    let Json(input) = input.map_err(|e| Failure::bad_request(e.body_text()))?;
    state
        .service
        .save_draft(&principal, &account, input)
        .await
        .map(Json)
        .map_err(Failure::from)
}

async fn send(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    input: Result<Json<DraftInput>, JsonRejection>,
) -> Result<Json<DeliveryOutcome>, Failure> {
    let account = account_header(&headers)?;
    let Json(input) = input.map_err(|e| Failure::bad_request(e.body_text()))?;
    state
        .service
        .send(&principal, &account, input)
        .await
        .map(Json)
        .map_err(Failure::from)
}

async fn list_send_requests(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
) -> Result<Json<Vec<approvals::PendingSend>>, Failure> {
    state
        .service
        .list_send_requests(&principal)
        .map(Json)
        .map_err(Failure::from)
}

async fn send_request(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    Path(id): Path<String>,
) -> Result<Json<SendStatus>, Failure> {
    state
        .service
        .send_request_status(&principal, &id, None)
        .map(Json)
        .map_err(Failure::from)
}

async fn approve_send(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    Path(id): Path<String>,
) -> Result<Json<approvals::PendingSend>, Failure> {
    state
        .service
        .approve_send(&principal, &id)
        .await
        .map(Json)
        .map_err(Failure::from)
}

async fn reject_send(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    Path(id): Path<String>,
) -> Result<Json<approvals::PendingSend>, Failure> {
    state
        .service
        .reject_send(&principal, &id)
        .map(Json)
        .map_err(Failure::from)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NavigateInput {
    target: String,
}

async fn navigate(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    input: Result<Json<NavigateInput>, JsonRejection>,
) -> Result<Json<NavigateResult>, Failure> {
    let account = account_header(&headers)?;
    let Json(input) = input.map_err(|_| Failure::bad_request("Invalid navigation request"))?;
    state
        .service
        .navigate(&principal, &account, &input.target)
        .map(Json)
        .map_err(Failure::from)
}

async fn ai_get_settings(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
) -> Result<Json<ai::SettingsView>, Failure> {
    let account = optional_account_header(&headers);
    state
        .service
        .ai_settings(&principal, account.as_deref())
        .map(Json)
        .map_err(Failure::from)
}

async fn ai_put_settings(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    input: Result<Json<ai::AiSettings>, JsonRejection>,
) -> Result<Json<ai::SettingsView>, Failure> {
    let account = optional_account_header(&headers);
    let Json(input) = input.map_err(|_| Failure::bad_request("Invalid AI settings"))?;
    state
        .service
        .ai_update_settings(&principal, account.as_deref(), input)
        .await
        .map(Json)
        .map_err(Failure::from)
}

async fn ai_put_key(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    input: Result<Json<ai::KeyInput>, JsonRejection>,
) -> Result<Json<ai::SettingsView>, Failure> {
    let Json(input) = input.map_err(|_| Failure::bad_request("Invalid key"))?;
    state
        .service
        .ai_put_key(&principal, input)
        .await
        .map(Json)
        .map_err(Failure::from)
}

async fn ai_test_connection(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    input: Result<Json<ai::TestInput>, JsonRejection>,
) -> Result<Json<serde_json::Value>, Failure> {
    let account = optional_account_header(&headers);
    let Json(input) = input.map_err(|_| Failure::bad_request("Invalid test"))?;
    let ok = state
        .service
        .ai_test_connection(&principal, account.as_deref(), input)
        .await?;
    Ok(Json(serde_json::json!({ "ok": ok })))
}

async fn ai_preview(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    input: Result<Json<ai::PreviewInput>, JsonRejection>,
) -> Result<Json<ai::PreviewResult>, Failure> {
    let account = account_header(&headers)?;
    let Json(input) = input.map_err(|_| Failure::bad_request("Invalid rules"))?;
    state
        .service
        .ai_preview(&principal, &account, input)
        .map(Json)
        .map_err(Failure::from)
}

async fn ai_reclassify(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    input: Result<Json<ai::ReclassifyInput>, JsonRejection>,
) -> Result<Json<ai::Reclassified>, Failure> {
    let account = account_header(&headers)?;
    let Json(input) = input.map_err(|_| Failure::bad_request("Invalid reclassify request"))?;
    state
        .service
        .ai_reclassify(&principal, &account, input)
        .await
        .map(Json)
        .map_err(Failure::from)
}

async fn ai_get_summary(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, Failure> {
    let account = account_header(&headers)?;
    let summary = state
        .service
        .ai_get_summary(&principal, &account, &id)
        .await?;
    Ok(Json(serde_json::json!({ "summary": summary })))
}

async fn ai_create_summary(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, Failure> {
    let account = account_header(&headers)?;
    let summary = state
        .service
        .ai_create_summary(&principal, &account, &id)
        .await?;
    Ok(Json(serde_json::json!({ "summary": summary })))
}

async fn ai_delete_summary(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, Failure> {
    let account = account_header(&headers)?;
    state
        .service
        .ai_delete_summary(&principal, &account, &id)
        .await?;
    Ok(Json(serde_json::json!({ "summary": null })))
}

async fn ai_draft_reply(
    State(state): State<AppState>,
    axum::Extension(principal): axum::Extension<Principal>,
    headers: HeaderMap,
    input: Result<Json<ai::ReplyInput>, JsonRejection>,
) -> Result<Json<serde_json::Value>, Failure> {
    let account = account_header(&headers)?;
    let Json(input) = input.map_err(|_| Failure::bad_request("Invalid reply request"))?;
    let text = state
        .service
        .ai_draft_reply(&principal, &account, input)
        .await?;
    Ok(Json(serde_json::json!({ "text": text })))
}
