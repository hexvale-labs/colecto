//! Trusted local HTTP adapter. Only built-in UI modules are loaded by this demo.
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Request, State, rejection::JsonRejection},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
};
use colecto_core::{Accounts, Error, MailStore, SecretStore};
use colecto_extension_api::{
    AccountInfo, AccountInput, ApiError, Draft, DraftInput, Message, SecurityInfo, SecurityInput,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tower_http::services::ServeDir;

#[derive(Clone)]
struct Host {
    accounts: Arc<Mutex<Accounts>>,
    authority: String,
    origin: String,
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
                eprintln!("Draft storage error: {error}");
                Self(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Could not save the draft. Your changes are still in the editor.".into(),
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
    let host = Host {
        accounts: Arc::new(Mutex::new(accounts)),
        origin: format!("http://{authority}"),
        authority,
    };
    let api = Router::new()
        .route("/accounts", get(list_accounts).post(upsert_account))
        .route(
            "/accounts/{id}/disconnect",
            axum::routing::post(disconnect_account),
        )
        .route("/security", get(security).post(security_action))
        .route("/account", get(account))
        .route("/send", axum::routing::post(send))
        .route("/messages", get(messages))
        .route("/messages/{id}", get(message))
        .route("/drafts", get(drafts).post(save_draft))
        .route("/drafts/{id}", get(draft))
        .fallback(|| async { Failure(StatusCode::NOT_FOUND, "API route not found".into()) });
    Router::new()
        .nest("/api/v1", api)
        .fallback_service(ServeDir::new(assets))
        .layer(DefaultBodyLimit::max(128_000))
        .layer(middleware::from_fn_with_state(host.clone(), boundary))
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

// shortcut: identities describe trusted bundled modules, not authentication for
// untrusted extensions; add an isolated runtime before loading third-party code.
fn grant(headers: &HeaderMap, operation: &str) -> Result<String, Failure> {
    let account = headers
        .get("x-colecto-account")
        .and_then(|v| v.to_str().ok());
    let extension = headers
        .get("x-colecto-extension")
        .and_then(|v| v.to_str().ok());
    if account.is_none() {
        return Err(forbidden());
    }
    let allowed = (operation == "account.read" && extension == Some("inbox"))
        || matches!(
            (extension, operation),
            (Some("inbox"), "messages.list" | "drafts.list")
                | (Some("reader"), "messages.read")
                | (
                    Some("composer"),
                    "drafts.read" | "drafts.write" | "messages.send"
                )
        );
    if allowed {
        Ok(account.unwrap().to_owned())
    } else {
        Err(forbidden())
    }
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
async fn messages(
    State(host): State<Host>,
    headers: HeaderMap,
) -> Result<Json<Vec<Message>>, Failure> {
    let account = grant(&headers, "messages.list")?;
    with_store(host, account, |s| s.refresh()).await.map(Json)
}
async fn message(
    State(host): State<Host>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Message>, Failure> {
    let account = grant(&headers, "messages.read")?;
    with_store(host, account, move |s| {
        s.message(&id).cloned().ok_or(Error::Missing)
    })
    .await
    .map(Json)
}
async fn drafts(State(host): State<Host>, headers: HeaderMap) -> Result<Json<Vec<Draft>>, Failure> {
    let account = grant(&headers, "drafts.list")?;
    with_store(host, account, |s| Ok(s.drafts().to_vec()))
        .await
        .map(Json)
}
async fn draft(
    State(host): State<Host>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Draft>, Failure> {
    let account = grant(&headers, "drafts.read")?;
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
    let account = grant(&headers, "drafts.write")?;
    let Json(input) = input.map_err(|error| Failure(StatusCode::BAD_REQUEST, error.body_text()))?;
    with_store(host, account, move |s| s.save_draft(input))
        .await
        .map(Json)
}

async fn account(State(host): State<Host>, headers: HeaderMap) -> Result<Json<bool>, Failure> {
    let account = grant(&headers, "account.read")?;
    with_store(host, account, |s| Ok(s.connected()))
        .await
        .map(Json)
}
async fn send(
    State(host): State<Host>,
    headers: HeaderMap,
    input: Result<Json<DraftInput>, JsonRejection>,
) -> Result<Json<bool>, Failure> {
    let account = grant(&headers, "messages.send")?;
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
