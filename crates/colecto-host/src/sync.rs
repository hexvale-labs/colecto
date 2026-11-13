//! Per-account sync status, refreshes that keep network work outside the store lock, and
//! content prefetch after each Inbox refresh.
use crate::Host;
use colecto_core::Error;
use colecto_extension_api::Message;
use serde::Serialize;
use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
    time::{Duration, Instant},
};

/// How an account learns about new mail right now.
#[derive(Clone, Copy, Serialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// The demo account: local sample mail, nothing to sync.
    Local,
    JmapPush,
    ImapIdle,
    Polling,
    /// Not connected, or the last refresh failed; `error` says why.
    Offline,
}

#[derive(Clone, Copy, Serialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Activity {
    Idle,
    Syncing,
    Prefetching,
}

/// `GET /api/v1/sync` lists one of these per account; the `sync` event carries one.
#[derive(Clone, Serialize, PartialEq, Eq, Debug)]
pub struct AccountSync {
    pub account: String,
    /// `imap`, `jmap`, or `demo`.
    pub protocol: &'static str,
    pub mode: Mode,
    /// Seconds between polls; polling also runs underneath push.
    pub interval: u64,
    pub activity: Activity,
    /// `(done, total)` messages for the current download, when the protocol can count them.
    pub progress: Option<(usize, usize)>,
    /// A short, provider-redacted reason when `mode` is `offline`.
    pub error: Option<String>,
    /// Unix seconds of the last successful Inbox refresh in this process.
    pub updated_at: Option<u64>,
}

#[derive(Default)]
struct Entry {
    push: bool,
    activity: Option<Activity>,
    progress: Option<(usize, usize)>,
    error: Option<String>,
    updated_at: Option<u64>,
    emitted: Option<Instant>,
}

/// Sync bookkeeping shared by the HTTP handlers and the background service.
pub(crate) struct SyncState {
    entries: Mutex<HashMap<String, Entry>>,
    interval: Mutex<u64>,
    /// Accounts with a refresh in flight; background passes skip them.
    refreshing: Mutex<HashSet<String>>,
    prefetching: Mutex<HashSet<String>>,
    /// Prefetch runs one account at a time so it never crowds out what the owner asked for.
    prefetch_slot: tokio::sync::Semaphore,
}
impl Default for SyncState {
    fn default() -> Self {
        Self {
            entries: Mutex::default(),
            interval: Mutex::new(60),
            refreshing: Mutex::default(),
            prefetching: Mutex::default(),
            prefetch_slot: tokio::sync::Semaphore::new(1),
        }
    }
}

/// Progress events are at most this frequent per account; start and finish always go out.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(200);

impl SyncState {
    pub(crate) fn set_interval(&self, interval: Duration) {
        if let Ok(mut value) = self.interval.lock() {
            *value = interval.as_secs().max(1);
        }
    }
    fn update(&self, host: &Host, account: &str, change: impl FnOnce(&mut Entry)) {
        let Ok(mut entries) = self.entries.lock() else {
            return;
        };
        let entry = entries.entry(account.to_owned()).or_default();
        let before = (
            entry.activity,
            entry.progress,
            entry.error.clone(),
            entry.push,
        );
        change(entry);
        if before
            == (
                entry.activity,
                entry.progress,
                entry.error.clone(),
                entry.push,
            )
        {
            return;
        }
        // Intermediate progress is throttled; state changes and the final count are not.
        let intermediate = matches!(entry.progress, Some((done, total)) if done > 0 && done < total)
            && before.0 == entry.activity;
        if intermediate
            && entry
                .emitted
                .is_some_and(|at| at.elapsed() < PROGRESS_INTERVAL)
        {
            return;
        }
        entry.emitted = Some(Instant::now());
        drop(entries);
        if let Some(status) = status(host, account) {
            let _ = host
                .events
                .send(("sync", serde_json::to_string(&status).unwrap_or_default()));
        }
    }
    pub(crate) fn set_push(&self, host: &Host, account: &str, push: bool) {
        self.update(host, account, |entry| entry.push = push);
    }
}

/// Accounts that background work (refresh, push watchers, prefetch) may touch. Every
/// background path goes through this one filter, so new eligibility rules go here.
pub(crate) fn eligible(host: &Host) -> Vec<String> {
    let Ok(accounts) = host.accounts.lock() else {
        return Vec::new();
    };
    accounts
        .list()
        .into_iter()
        .filter(|account| account.connected)
        .map(|account| account.id)
        .collect()
}

/// The current status of every account the Host knows.
pub(crate) fn statuses(host: &Host) -> Vec<AccountSync> {
    let ids: Vec<String> = host
        .accounts
        .lock()
        .map(|accounts| accounts.list().into_iter().map(|a| a.id).collect())
        .unwrap_or_default();
    ids.iter().filter_map(|id| status(host, id)).collect()
}

fn status(host: &Host, account: &str) -> Option<AccountSync> {
    let info = host
        .accounts
        .lock()
        .ok()?
        .list()
        .into_iter()
        .find(|a| a.id == account)?;
    let interval = host.sync.interval.lock().map(|i| *i).unwrap_or(60);
    let entries = host.sync.entries.lock().ok()?;
    let empty = Entry::default();
    let entry = entries.get(account).unwrap_or(&empty);
    let protocol = match &info.settings {
        None => "demo",
        Some(settings) if settings.protocol == colecto_extension_api::Protocol::Jmap => "jmap",
        Some(_) => "imap",
    };
    let (mode, error) = mode(protocol, info.connected, entry);
    Some(AccountSync {
        account: account.to_owned(),
        protocol,
        mode,
        interval,
        activity: entry.activity.unwrap_or(Activity::Idle),
        progress: entry.progress,
        error,
        updated_at: entry.updated_at,
    })
}

fn mode(protocol: &str, connected: bool, entry: &Entry) -> (Mode, Option<String>) {
    if protocol == "demo" {
        (Mode::Local, None)
    } else if !connected {
        (Mode::Offline, Some("Not connected".to_owned()))
    } else if let Some(error) = &entry.error {
        (Mode::Offline, Some(error.clone()))
    } else if entry.push && protocol == "jmap" {
        (Mode::JmapPush, None)
    } else if entry.push {
        (Mode::ImapIdle, None)
    } else {
        (Mode::Polling, None)
    }
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Whether a refresh was asked for or runs in the background.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Origin {
    Owner,
    Background,
}

/// List a folder. The store lock is held only to prepare and to apply the result, never
/// while talking to the provider, so opening a message never waits behind a refresh.
/// Returns `Ok(None)` when a background refresh found another one already running.
pub(crate) fn refresh(
    host: &Host,
    account: &str,
    folder: &str,
    search: &str,
    origin: Origin,
) -> Result<Option<Vec<Message>>, Error> {
    let store = host
        .accounts
        .lock()
        .map_err(|_| Error::Missing)?
        .store(account)?;
    let job = {
        let mut guard = store.lock().map_err(|_| Error::Missing)?;
        match guard.begin_query(folder, search)? {
            Some(job) => job,
            None => return guard.query(folder, search).map(Some),
        }
    };
    // Only plain folder refreshes count as syncing; a search is a lookup.
    let tracked = job.search().is_empty();
    if tracked {
        let started = host
            .sync
            .refreshing
            .lock()
            .map_err(|_| Error::Missing)?
            .insert(account.to_owned());
        if !started && origin == Origin::Background {
            return Ok(None);
        }
        host.sync.update(host, account, |entry| {
            entry.activity = Some(Activity::Syncing);
            entry.progress = None;
        });
    }
    let result = job
        .run(&mut |done, total| {
            if tracked && total > 0 {
                host.sync
                    .update(host, account, |entry| entry.progress = Some((done, total)));
            }
        })
        .and_then(|listing| {
            store
                .lock()
                .map_err(|_| Error::Missing)?
                .finish_query(job, listing)
        });
    if tracked {
        if let Ok(mut refreshing) = host.sync.refreshing.lock() {
            refreshing.remove(account);
        }
        host.sync.update(host, account, |entry| {
            entry.activity = None;
            entry.progress = None;
            match &result {
                Ok(_) => {
                    entry.error = None;
                    if folder == "inbox" {
                        entry.updated_at = Some(now());
                    }
                }
                // Transport and validation errors are already redacted for display.
                Err(error @ (Error::Transport(_) | Error::Invalid(_))) => {
                    entry.error = Some(error.to_string())
                }
                Err(_) => entry.error = Some("Mail could not be refreshed".into()),
            }
        });
    }
    result.map(Some)
}

/// After an Inbox refresh, download the newest messages that are not cached yet, so
/// opening them needs no network. Runs on a blocking thread, one account at a time.
pub(crate) fn start_prefetch(host: &Host, account: &str) {
    let preferences = match host.preferences.lock() {
        Ok(preferences) => preferences.get().prefetch.clone(),
        Err(_) => return,
    };
    if !preferences.enabled
        || preferences.count == 0
        || !eligible(host).iter().any(|a| a == account)
    {
        return;
    }
    if !host
        .sync
        .prefetching
        .lock()
        .is_ok_and(|mut running| running.insert(account.to_owned()))
    {
        return;
    }
    let (host, account) = (host.clone(), account.to_owned());
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        return;
    };
    runtime.spawn(async move {
        if let Ok(_slot) = host.sync.prefetch_slot.acquire().await {
            let inner = host.clone();
            let id = account.clone();
            let count = usize::from(preferences.count);
            let _ = tokio::task::spawn_blocking(move || prefetch(&inner, &id, count)).await;
        }
        if let Ok(mut running) = host.sync.prefetching.lock() {
            running.remove(&account);
        }
    });
}

fn prefetch(host: &Host, account: &str, count: usize) {
    let Some((reader, ids)) = (|| {
        let accounts = host.accounts.lock().ok()?;
        let ids: Vec<String> = accounts
            .cached(account, "inbox")
            .ok()?
            .messages
            .into_iter()
            .take(count)
            .map(|message| message.id)
            .collect();
        let reader = accounts.store(account).ok()?.lock().ok()?.reader()?;
        Some((reader, ids))
    })() else {
        return;
    };
    let result = reader.prefetch(&ids, &mut |done, total| {
        host.sync.update(host, account, |entry| {
            entry.activity = Some(Activity::Prefetching);
            entry.progress = Some((done, total));
        });
    });
    host.sync.update(host, account, |entry| {
        if entry.activity == Some(Activity::Prefetching) {
            entry.activity = None;
            entry.progress = None;
        }
    });
    match result {
        Ok(count) if count > 0 => tracing::debug!(%account, count, "Prefetched messages"),
        Ok(_) => {}
        Err(error) => tracing::debug!(%account, %error, "Prefetch stopped"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn host(dir: &std::path::Path) -> Host {
        let accounts = colecto_core::Accounts::open(
            colecto_core::MailStore::open(dir).unwrap(),
            Arc::new(crate::UnavailableSecrets),
        )
        .unwrap();
        let database = accounts.database();
        Host {
            events: tokio::sync::broadcast::channel(16).0,
            bus_replies: Default::default(),
            keys: Arc::new(Mutex::new(
                crate::api_keys::ApiKeys::open(database.clone()).unwrap(),
            )),
            preferences: Arc::new(Mutex::new(
                crate::preferences::PreferenceStore::open(database.clone()).unwrap(),
            )),
            approvals: Arc::new(Mutex::new(
                crate::approvals::Approvals::open(database.clone(), crate::api_keys::now())
                    .unwrap(),
            )),
            status_polls: Arc::default(),
            accounts: Arc::new(Mutex::new(accounts)),
            extensions: Arc::new(Mutex::new(
                crate::extensions::Extensions::open(database).unwrap(),
            )),
            authority: "127.0.0.1:4173".into(),
            origin: "http://127.0.0.1:4173".into(),
            execution: Arc::new(tokio::sync::Semaphore::new(2)),
            sync: Arc::default(),
        }
    }

    #[test]
    fn modes_follow_connection_failures_and_push() {
        let mut entry = Entry::default();
        assert_eq!(mode("demo", false, &entry).0, Mode::Local);
        assert_eq!(
            mode("imap", false, &entry),
            (Mode::Offline, Some("Not connected".into()))
        );
        assert_eq!(mode("imap", true, &entry).0, Mode::Polling);
        entry.push = true;
        assert_eq!(mode("imap", true, &entry).0, Mode::ImapIdle);
        assert_eq!(mode("jmap", true, &entry).0, Mode::JmapPush);
        entry.error = Some("Mail server timed out".into());
        assert_eq!(
            mode("jmap", true, &entry),
            (Mode::Offline, Some("Mail server timed out".into()))
        );
    }

    #[test]
    fn progress_is_reported_as_sync_events_and_throttled() {
        let dir = tempfile::tempdir().unwrap();
        let host = host(dir.path());
        host.sync.set_interval(Duration::from_secs(90));
        let mut events = host.events.subscribe();
        let next = |events: &mut tokio::sync::broadcast::Receiver<(&'static str, String)>| {
            let (name, data) = events.try_recv().ok()?;
            assert_eq!(name, "sync");
            serde_json::from_str::<serde_json::Value>(&data).ok()
        };
        host.sync.update(&host, "demo", |entry| {
            entry.activity = Some(Activity::Syncing)
        });
        let event = next(&mut events).unwrap();
        assert_eq!(event["activity"], "syncing");
        assert_eq!(event["mode"], "local");
        assert_eq!(event["interval"], 90);
        for done in 1..5 {
            host.sync
                .update(&host, "demo", |entry| entry.progress = Some((done, 10)));
        }
        // Counts that follow the start faster than the throttle allows are dropped.
        assert!(next(&mut events).is_none());
        host.sync
            .update(&host, "demo", |entry| entry.progress = Some((10, 10)));
        assert_eq!(
            next(&mut events).unwrap()["progress"],
            serde_json::json!([10, 10])
        );
        // Nothing changed, nothing sent.
        host.sync
            .update(&host, "demo", |entry| entry.progress = Some((10, 10)));
        assert!(next(&mut events).is_none());
        let all = statuses(&host);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].activity, Activity::Syncing);
    }

    #[tokio::test]
    async fn demo_refresh_needs_no_provider_and_prefetch_skips_it() {
        let dir = tempfile::tempdir().unwrap();
        let host = host(dir.path());
        let messages = refresh(&host, "demo", "inbox", "", Origin::Owner)
            .unwrap()
            .unwrap();
        assert_eq!(messages.len(), 4);
        // The demo account is never connected, so background work leaves it alone.
        assert!(eligible(&host).is_empty());
        start_prefetch(&host, "demo");
        assert!(host.sync.prefetching.lock().unwrap().is_empty());
    }
}
