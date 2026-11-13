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
    sync::{Arc, Mutex},
    time::Duration,
};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct AccessPolicy {
    authority: String,
    pub service_command: Option<Vec<String>>,
    pub daemon_socket: bool,
    local: bool,
    origin: String,
    token: Option<Arc<Zeroizing<String>>>,
}
impl AccessPolicy {
    pub fn local(authority: String) -> Self {
        Self {
            service_command: None,
            daemon_socket: false,
            local: true,
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
        if !valid_token(&token) {
            return Err(
                "Access token must contain 64–512 ASCII letters, digits, hyphens or underscores"
                    .into(),
            );
        }
        Ok(Self {
            service_command: None,
            daemon_socket: false,
            local: false,
            authority: origin.strip_prefix("https://").unwrap().into(),
            origin,
            token: Some(Arc::new(Zeroizing::new(token))),
        })
    }
}
/// Owner tokens are 64–512 ASCII letters, digits, hyphens or underscores.
pub(crate) fn valid_token(token: &str) -> bool {
    (64..=512).contains(&token.len())
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
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
    if !request.uri().path().starts_with("/api/") || request.uri().path() == "/api/v1/version" {
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
                | "/api/v1/folders"
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
    events: crate::Events,
    snapshots: Arc<Mutex<NotificationSnapshots>>,
    /// Accounts with a running push watcher, and provider keys known not to support push.
    watching: Arc<Mutex<Watching>>,
    closing: tokio::sync::watch::Sender<bool>,
}
impl MailService {
    pub fn new(
        accounts: Accounts,
        policy: AccessPolicy,
        assets: impl Into<crate::Assets>,
    ) -> Result<Self, String> {
        let assets = assets.into();
        if let crate::Assets::Directory(directory) = &assets {
            match std::fs::canonicalize(directory) {
                Ok(root) if accounts.data_directory().starts_with(&root) => {
                    return Err("Static assets must not contain the private data directory".into());
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(_) => return Err("Cannot validate static assets directory".into()),
            }
        }
        let control = crate::services::Control::open(
            accounts.data_directory(),
            policy.local && !policy.daemon_socket,
            policy.service_command.clone(),
        )?;
        let mut policy = policy;
        if policy.token.is_none() {
            policy.token = Some(Arc::new(Zeroizing::new(
                crate::runtime::local_owner_token(accounts.data_directory())
                    .map_err(|error| error.to_string())?,
            )));
        }
        let database = accounts.database();
        let extensions = crate::extensions::Extensions::open(database.clone())?;
        let keys = Arc::new(Mutex::new(ApiKeys::open(database.clone())?));
        let preferences = Arc::new(Mutex::new(crate::preferences::PreferenceStore::open(
            database.clone(),
        )?));
        let auth = Authentication {
            policy: policy.clone(),
            keys: keys.clone(),
        };
        let approvals = Arc::new(Mutex::new(crate::approvals::Approvals::open(
            database,
            api_keys::now(),
        )?));
        let (events, _) = tokio::sync::broadcast::channel(64);
        let host = Host {
            events: events.clone(),
            bus_replies: Default::default(),
            approvals,
            preferences,
            status_polls: Arc::default(),
            keys,
            accounts: Arc::new(Mutex::new(accounts)),
            extensions: Arc::new(Mutex::new(extensions)),
            authority: policy.authority.clone(),
            origin: policy.origin.clone(),
            execution: Arc::new(tokio::sync::Semaphore::new(2)),
            sync: Arc::default(),
        };
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
                                    Ok((_, data)) if data.is_empty() => None,
                                    Ok((name, data)) => Some((
                                        Ok(Event::default().event(name).data(data)),
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
            .layer(middleware::from_fn_with_state(
                control,
                crate::services::handle,
            ))
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
            watching: Arc::default(),
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
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<(&'static str, String)> {
        self.events.subscribe()
    }
    /// One bounded polling pass; failed or busy accounts don't block unrelated accounts.
    pub async fn refresh_once(&self) {
        let (_stop, receiver) = tokio::sync::watch::channel(false);
        self.refresh_until_stopped(receiver, None).await;
    }
    async fn refresh_until_stopped(
        &self,
        stop: tokio::sync::watch::Receiver<bool>,
        only: Option<&str>,
    ) {
        if let Ok(mut approvals) = self.host.approvals.lock()
            && approvals.maintain(api_keys::now()).is_err()
        {
            tracing::warn!("Could not expire retained delivery requests");
        }
        let accounts = crate::sync::eligible(&self.host)
            .into_iter()
            .filter(|id| only.is_none_or(|only| only == id));
        let mut tasks = tokio::task::JoinSet::new();
        for account in accounts {
            let host = self.host.clone();
            let events = self.events.clone();
            let snapshots = self.snapshots.clone();
            let watching = self.watching.clone();
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
                    let refreshed = match crate::sync::refresh(
                        &host,
                        &account,
                        "inbox",
                        "",
                        crate::sync::Origin::Background,
                    ) {
                        // Another refresh of this account is already running.
                        Ok(None) => return,
                        Ok(Some(messages)) => Ok(messages),
                        Err(error) => Err(error),
                    };
                    if let Ok(mut watching) = watching.lock() {
                        // Push needs a working connection; failing accounts stay on polling alone.
                        if refreshed.is_ok() {
                            watching.healthy.insert(account.clone());
                        } else {
                            watching.healthy.remove(&account);
                        }
                    }
                    match refreshed {
                        Ok(messages) => {
                            tracing::debug!(%account, count = messages.len(), "Refreshed mail");
                            crate::sync::start_prefetch(&host, &account);
                            if let Ok(mut snapshots) = snapshots.lock()
                                && let Some(event) = snapshots
                                    .observe(&account, messages.iter().map(|m| m.id.clone()))
                            {
                                let _ = events.send(("mail", event));
                            }
                        }
                        Err(error) => {
                            tracing::warn!(%account, %error, "Background receive failed")
                        }
                    }
                })
                .await;
                if let Err(error) = result {
                    tracing::error!(%error, "Background mail task failed");
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
        self.host.sync.set_interval(interval);
        loop {
            if *stop.borrow() {
                break;
            }
            self.refresh_until_stopped(stop.clone(), None).await;
            self.watch_push(&stop);
            tokio::select! { _ = tokio::time::sleep(interval) => {}, result = stop.changed() => { if result.is_err() || *stop.borrow() { break; } } }
        }
        let _ = self.closing.send(true);
        let _ = self.events.send(("", String::new()));
    }
    /// Start an IMAP IDLE or JMAP push watcher for each connected account that lacks one.
    /// Watchers refresh their account as soon as the provider reports a change; polling
    /// continues underneath, so a failed or unsupported watcher only costs latency.
    fn watch_push(&self, stop: &tokio::sync::watch::Receiver<bool>) {
        let candidates = {
            let eligible = crate::sync::eligible(&self.host);
            let Ok(accounts) = self.host.accounts.lock() else {
                return;
            };
            eligible
                .into_iter()
                .filter_map(|id| accounts.store(&id).ok().map(|s| (id, s)))
                .collect::<Vec<_>>()
        };
        let healthy = self
            .watching
            .lock()
            .map(|w| w.healthy.clone())
            .unwrap_or_default();
        for (account, store) in candidates {
            if !healthy.contains(&account) {
                continue;
            }
            let Some(watcher) = store.lock().ok().and_then(|s| s.push_watcher()) else {
                continue;
            };
            {
                let Ok(mut watching) = self.watching.lock() else {
                    return;
                };
                if watching.unsupported.contains(watcher.key())
                    || !watching.running.insert(account.clone())
                {
                    continue;
                }
            }
            let service = self.clone();
            let stop = stop.clone();
            let runtime = tokio::runtime::Handle::current();
            // A detached thread: an idle connection must never delay shutdown.
            std::thread::spawn(move || {
                service.host.sync.set_push(&service.host, &account, true);
                while !*stop.borrow() {
                    match watcher.wait(PUSH_WAIT) {
                        Ok(true) => runtime
                            .block_on(service.refresh_until_stopped(stop.clone(), Some(&account))),
                        Ok(false) => {}
                        Err(colecto_core::Error::Invalid(_)) => {
                            if let Ok(mut watching) = service.watching.lock() {
                                watching.unsupported.insert(watcher.key().to_owned());
                            }
                            break;
                        }
                        Err(error) => {
                            tracing::warn!(%account, %error, "Push notifications paused");
                            break;
                        }
                    }
                    // Stop when the account was disconnected or reconfigured.
                    let current = service
                        .host
                        .accounts
                        .lock()
                        .ok()
                        .and_then(|accounts| accounts.store(&account).ok())
                        .and_then(|store| store.lock().ok().and_then(|s| s.push_watcher()));
                    if current.is_none_or(|current| current.key() != watcher.key()) {
                        break;
                    }
                }
                if let Ok(mut watching) = service.watching.lock() {
                    watching.running.remove(&account);
                }
                service.host.sync.set_push(&service.host, &account, false);
            });
        }
    }
}

/// Providers may drop idle connections after 30 minutes; re-checking sooner also notices
/// disconnected accounts.
const PUSH_WAIT: Duration = Duration::from_secs(10 * 60);

#[derive(Default)]
struct Watching {
    /// Accounts whose last refresh succeeded.
    healthy: std::collections::HashSet<String>,
    running: std::collections::HashSet<String>,
    unsupported: std::collections::HashSet<String>,
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
