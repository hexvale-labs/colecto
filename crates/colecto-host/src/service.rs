//! Reusable mail service, independent of the UI process and service supervisor.
use crate::{Host, build_app};
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
async fn authenticate(
    State(policy): State<AccessPolicy>,
    request: Request,
    next: Next,
) -> Response {
    if request.uri().path().starts_with("/api/")
        && let Some(token) = policy.token
    {
        let supplied = request
            .headers()
            .get(header::AUTHORIZATION)
            .and_then(|h| h.to_str().ok())
            .and_then(|h| h.strip_prefix("Bearer "))
            .unwrap_or("");
        if supplied.as_bytes().ct_eq(token.as_bytes()).unwrap_u8() != 1 {
            return (
                StatusCode::UNAUTHORIZED,
                axum::Json(serde_json::json!({"error":"Enter the daemon access token to connect"})),
            )
                .into_response();
        }
    }
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
        let extensions = crate::extensions::Extensions::open(accounts.data_directory())
            .map_err(|e| e.to_string())?;
        let host = Host {
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
            .layer(middleware::from_fn_with_state(
                host.clone(),
                crate::boundary,
            ))
            .layer(middleware::from_fn_with_state(policy, authenticate));
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
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<String> {
        self.events.subscribe()
    }
    /// One bounded polling pass; failed or busy accounts don't block unrelated accounts.
    pub async fn refresh_once(&self) {
        let (_stop, receiver) = tokio::sync::watch::channel(false);
        self.refresh_until_stopped(receiver).await;
    }
    async fn refresh_until_stopped(&self, stop: tokio::sync::watch::Receiver<bool>) {
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
