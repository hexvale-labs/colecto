//! Reusable mail service, independent of the UI process and service supervisor.
use crate::{
    Host,
    api_keys::{self, ApiKeys, Principal},
    build_app,
};
use axum::{
    Router,
    extract::{Request, State},
    http::{StatusCode, header},
    middleware::{self, Next},
    response::{
        IntoResponse, Response, Sse,
        sse::{Event, KeepAlive},
    },
    routing::get,
};
use colecto_core::Accounts;
use std::{
    convert::Infallible,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct AccessPolicy {
    authority: String,
    origin: String,
    token: Option<Arc<Zeroizing<String>>>,
}
impl AccessPolicy {
    pub fn local(authority: String) -> Self {
        Self {
            origin: format!("http://{authority}"),
            authority,
            token: None,
        }
    }
    pub fn remote(origin: &str, token: String) -> Result<Self, String> {
        let url = reqwest::Url::parse(origin).map_err(|_| "Invalid public origin")?;
        if url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.path() != "/"
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err(
                "Public origin must be an HTTPS origin without path, credentials, or query".into(),
            );
        }
        let origin = url.origin().ascii_serialization();
        if token.len() < 64
            || token.len() > 512
            || !token
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
        {
            return Err(
                "Access token must contain 64–512 ASCII letters, digits, hyphens or underscores"
                    .into(),
            );
        }
        Ok(Self {
            authority: origin.strip_prefix("https://").unwrap().into(),
            origin,
            token: Some(Arc::new(Zeroizing::new(token))),
        })
    }
}
#[derive(Clone)]
struct Authentication {
    policy: AccessPolicy,
    keys: Arc<Mutex<ApiKeys>>,
}
/// MCP stays off until the owner turns it on, and answers as if it did not exist meanwhile.
async fn mcp_enabled(State(host): State<Host>, request: Request, next: Next) -> Response {
    if host.preferences.lock().is_ok_and(|p| p.get().mcp) {
        next.run(request).await
    } else {
        (
            StatusCode::NOT_FOUND,
            axum::Json(serde_json::json!({"error":"MCP is turned off. Turn it on in Settings → API keys."})),
        )
            .into_response()
    }
}
async fn authenticate(
    State(auth): State<Authentication>,
    mut request: Request,
    next: Next,
) -> Response {
    if !request.uri().path().starts_with("/api/") {
        return next.run(request).await;
    }
    let session_request = request.uri().path() == "/api/v1/session";
    let headers = request.headers();
    let bearer = headers.get(header::AUTHORIZATION);
    let principal = if let Some(bearer) = bearer {
        let supplied = bearer
            .to_str()
            .ok()
            .and_then(|h| h.strip_prefix("Bearer "))
            .unwrap_or("");
        if auth
            .policy
            .token
            .as_ref()
            .is_some_and(|token| supplied.as_bytes().ct_eq(token.as_bytes()).unwrap_u8() == 1)
        {
            Some(Principal::Owner)
        } else {
            auth.keys
                .lock()
                .ok()
                .and_then(|mut keys| keys.authenticate(supplied, api_keys::now()).ok())
        }
    } else {
        None
    };
    let Some(principal) = principal else {
        return (
            StatusCode::UNAUTHORIZED,
            axum::Json(serde_json::json!({"error":"Enter the daemon access token to connect"})),
        )
            .into_response();
    };
    if matches!(principal, Principal::ApiKey(_)) {
        let path = request.uri().path();
        if !matches!(
            path,
            "/api/v1/account"
                | "/api/v1/messages"
                | "/api/v1/drafts"
                | "/api/v1/labels"
                | "/api/v1/send"
                | "/api/v1/mcp"
        ) && !path.starts_with("/api/v1/messages/")
            && !path.starts_with("/api/v1/drafts/")
            && !(request.method() == axum::http::Method::GET
                && path.starts_with("/api/v1/send-requests/"))
        {
            return crate::forbidden().into_response();
        }
    }
    if session_request {
        if request.method() != axum::http::Method::POST {
            return StatusCode::METHOD_NOT_ALLOWED.into_response();
        }
        return axum::Json(true).into_response();
    }

    request.extensions_mut().insert(principal);
    next.run(request).await
}

#[derive(Clone)]
pub struct MailService {
    host: Host,
    router: Router,
    events: tokio::sync::broadcast::Sender<String>,
    snapshots: Arc<Mutex<NotificationSnapshots>>,
    closing: tokio::sync::watch::Sender<bool>,
}
impl MailService {
    pub fn new(accounts: Accounts, policy: AccessPolicy, assets: PathBuf) -> Result<Self, String> {
        match std::fs::canonicalize(&assets) {
            Ok(root) if accounts.data_directory().starts_with(&root) => {
                return Err("Static assets must not contain the private data directory".into());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err("Cannot validate static assets directory".into()),
        }
        let mut policy = policy;
        if policy.token.is_none() {
            policy.token = Some(Arc::new(Zeroizing::new(
                crate::runtime::local_owner_token(accounts.data_directory())
                    .map_err(|error| error.to_string())?,
            )));
        }
        let database = accounts.database();
        let directory = accounts.data_directory();
        let extensions = crate::extensions::Extensions::open(database.clone(), directory)?;
        let keys = Arc::new(Mutex::new(ApiKeys::open(database.clone(), directory)?));
        let preferences = Arc::new(Mutex::new(crate::preferences::PreferenceStore::open(
            database.clone(),
            directory,
        )?));
        let auth = Authentication {
            policy: policy.clone(),
            keys: keys.clone(),
        };
        let approvals = Arc::new(Mutex::new(crate::approvals::Approvals::open(
            database,
            directory,
            api_keys::now(),
        )?));
        let host = Host {
            approvals,
            preferences,
            status_polls: Arc::default(),
            keys,
            accounts: Arc::new(Mutex::new(accounts)),
            extensions: Arc::new(Mutex::new(extensions)),
            authority: policy.authority.clone(),
            origin: policy.origin.clone(),
            execution: Arc::new(tokio::sync::Semaphore::new(2)),
        };
        let (events, _) = tokio::sync::broadcast::channel::<String>(64);
        let stream_events = events.clone();
        let (closing, stopped) = tokio::sync::watch::channel(false);
        let router = build_app(host.clone(), assets)
            .merge(
                Router::new()
                    .nest_service("/api/v1/mcp", crate::mcp::http(host.clone()))
                    .layer(middleware::from_fn_with_state(host.clone(), mcp_enabled)),
            )
            .route(
                "/api/v1/events",
                get(move |headers: axum::http::HeaderMap| {
                    let receiver = stream_events.subscribe();
                    let stopped = stopped.clone();
                    async move {
                        crate::host_grant(&headers)?;
                        let stream = futures_util::stream::unfold(
                            (receiver, stopped, true),
                            |(mut receiver, mut stopped, first)| async move {
                                if *stopped.borrow() {
                                    return None;
                                }
                                if first {
                                    return Some((
                                        Ok::<_, Infallible>(
                                            Event::default().event("ready").data("{}"),
                                        ),
                                        (receiver, stopped, false),
                                    ));
                                }
                                let event = tokio::select! {
                                    _ = stopped.changed() => return None,
                                    event = receiver.recv() => event,
                                };
                                match event {
                                    Ok(data) if data.is_empty() => None,
                                    Ok(data) => Some((
                                        Ok(Event::default().event("mail").data(data)),
                                        (receiver, stopped, false),
                                    )),
                                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                                        Some((
                                            Ok(Event::default().event("resync").data("{}")),
                                            (receiver, stopped, false),
                                        ))
                                    }
                                    Err(_) => None,
                                }
                            },
                        );
                        Ok::<_, crate::Failure>(Sse::new(stream).keep_alive(KeepAlive::default()))
                    }
                }),
            )
            .layer(middleware::from_fn_with_state(auth, authenticate))
            .layer(middleware::from_fn_with_state(
                host.clone(),
                crate::boundary,
            ));
        Ok(Self {
            host,
            router,
            events,
            snapshots: Arc::new(Mutex::new(NotificationSnapshots::default())),
            closing,
        })
    }
    pub fn router(&self) -> Router {
        self.router.clone()
    }
    /// The shared application database, for callers that own this service's lifecycle.
    pub fn database(&self) -> colecto_core::Database {
        self.host
            .accounts
            .lock()
            .map_or_else(|e| e.into_inner().database(), |a| a.database())
    }
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<String> {
        self.events.subscribe()
    }
    /// One bounded polling pass; failed or busy accounts don't block unrelated accounts.
    pub async fn refresh_once(&self) {
        let (_stop, receiver) = tokio::sync::watch::channel(false);
        self.refresh_until_stopped(receiver).await;
    }
    async fn refresh_until_stopped(&self, stop: tokio::sync::watch::Receiver<bool>) {
        if let Ok(mut approvals) = self.host.approvals.lock()
            && approvals.maintain(api_keys::now()).is_err()
        {
            eprintln!("Could not expire retained delivery requests");
        }
        let stores = {
            let Ok(accounts) = self.host.accounts.lock() else {
                return;
            };
            accounts
                .list()
                .into_iter()
                .filter(|a| a.connected)
                .filter_map(|a| accounts.store(&a.id).ok().map(|s| (a.id, s)))
                .collect::<Vec<_>>()
        };
        let mut tasks = tokio::task::JoinSet::new();
        for (account, store) in stores {
            let events = self.events.clone();
            let snapshots = self.snapshots.clone();
            let semaphore = self.host.execution.clone();
            let stop = stop.clone();
            tasks.spawn(async move {
                let Some(_permit) = acquire_for_refresh(semaphore, stop.clone()).await else {
                    return;
                };
                let result = tokio::task::spawn_blocking(move || {
                    if *stop.borrow() {
                        return;
                    }
                    let Ok(mut store) = store.try_lock() else {
                        return;
                    };
                    match store.refresh() {
                        Ok(messages) => {
                            if let Ok(mut snapshots) = snapshots.lock()
                                && let Some(event) = snapshots
                                    .observe(&account, messages.iter().map(|m| m.id.clone()))
                            {
                                let _ = events.send(event);
                            }
                        }
                        Err(error) => eprintln!("Background receive failed for {account}: {error}"),
                    }
                })
                .await;
                if let Err(error) = result {
                    eprintln!("Background mail task failed: {error}");
                }
            });
        }
        while tasks.join_next().await.is_some() {}
    }
    pub async fn run_background(
        self,
        interval: Duration,
        mut stop: tokio::sync::watch::Receiver<bool>,
    ) {
        loop {
            if *stop.borrow() {
                break;
            }
            self.refresh_until_stopped(stop.clone()).await;
            tokio::select! { _ = tokio::time::sleep(interval) => {}, result = stop.changed() => { if result.is_err() || *stop.borrow() { break; } } }
        }
        let _ = self.closing.send(true);
        let _ = self.events.send(String::new());
    }
}

async fn acquire_for_refresh(
    semaphore: Arc<tokio::sync::Semaphore>,
    mut stop: tokio::sync::watch::Receiver<bool>,
) -> Option<tokio::sync::OwnedSemaphorePermit> {
    loop {
        if *stop.borrow() {
            return None;
        }
        tokio::select! {
            biased;
            changed = stop.changed() => {
                if changed.is_err() || *stop.borrow() { return None; }
            }
            permit = semaphore.clone().acquire_owned() => return permit.ok(),
        }
    }
}

#[derive(Default)]
struct NotificationSnapshots(std::collections::HashMap<String, std::collections::HashSet<String>>);
impl NotificationSnapshots {
    fn observe(&mut self, account: &str, ids: impl Iterator<Item = String>) -> Option<String> {
        let current = ids.collect::<std::collections::HashSet<_>>();
        let previous = self.0.insert(account.into(), current.clone())?;
        let mut ids = current.difference(&previous).collect::<Vec<_>>();
        ids.sort();
        (!ids.is_empty()).then(|| serde_json::json!({"account":account, "ids":ids}).to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn queued_refresh_never_starts_after_shutdown() {
        let semaphore = Arc::new(tokio::sync::Semaphore::new(0));
        let (stop, receiver) = tokio::sync::watch::channel(false);
        let worker = tokio::spawn(acquire_for_refresh(semaphore.clone(), receiver));
        tokio::task::yield_now().await;
        stop.send(true).unwrap();
        semaphore.add_permits(1);
        assert!(worker.await.unwrap().is_none());
        assert_eq!(semaphore.available_permits(), 1);
    }
    #[test]
    fn background_snapshot_is_independent_of_client_cache_and_empty_baselines() {
        let mut snapshots = NotificationSnapshots::default();
        assert!(snapshots.observe("a", ["old".into()].into_iter()).is_none());
        // The client can independently refresh its store; only background observations advance this baseline.
        let event = snapshots
            .observe("a", ["old".into(), "new".into()].into_iter())
            .unwrap();
        let event: serde_json::Value = serde_json::from_str(&event).unwrap();
        assert_eq!(event, serde_json::json!({"account":"a", "ids":["new"]}));
        assert!(
            snapshots
                .observe("a", ["old".into(), "new".into()].into_iter())
                .is_none()
        );
        assert!(snapshots.observe("b", std::iter::empty()).is_none());
        assert!(
            snapshots
                .observe("b", ["first".into()].into_iter())
                .is_some()
        );
    }
}
