//! Per-account sync status, refreshes that keep network work outside the store lock, and
//! content prefetch after each Inbox refresh.
use crate::ServiceInner;
use crate::events::ServiceEvent;
use colecto_core::{Error, Message, Protocol};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

/// How an account learns about new mail right now.
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Debug)]
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

#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum Activity {
    Idle,
    Syncing,
    Prefetching,
}

/// `GET /api/v1/sync` lists one of these per account; the `sync` event carries one.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct AccountSync {
    pub account: String,
    /// `imap`, `jmap`, or `demo`.
    pub protocol: String,
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

/// Sync bookkeeping shared by callers and the background service.
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

    fn update(&self, inner: &ServiceInner, account: &str, change: impl FnOnce(&mut Entry)) {
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
        if let Some(status) = status(inner, account) {
            let _ = inner.events.send(ServiceEvent::Sync(status));
        }
    }

    pub(crate) fn set_push(&self, inner: &ServiceInner, account: &str, push: bool) {
        self.update(inner, account, |entry| entry.push = push);
    }
}

/// Accounts that background work (refresh, push watchers, prefetch) may touch.
pub(crate) fn eligible(inner: &ServiceInner) -> Vec<String> {
    let Ok(preferences) = inner.preferences.lock().map(|p| p.get().clone()) else {
        return Vec::new();
    };
    let Ok(accounts) = inner.accounts.lock() else {
        return Vec::new();
    };
    accounts
        .list()
        .into_iter()
        .filter(|account| account.connected && preferences.account_enabled(&account.id))
        .map(|account| account.id)
        .collect()
}

/// The current status of every account the service knows.
pub(crate) fn statuses(inner: &ServiceInner) -> Vec<AccountSync> {
    let ids: Vec<String> = inner
        .accounts
        .lock()
        .map(|accounts| accounts.list().into_iter().map(|a| a.id).collect())
        .unwrap_or_default();
    ids.iter().filter_map(|id| status(inner, id)).collect()
}

fn status(inner: &ServiceInner, account: &str) -> Option<AccountSync> {
    let info = inner
        .accounts
        .lock()
        .ok()?
        .list()
        .into_iter()
        .find(|a| a.id == account)?;
    let interval = inner.sync.interval.lock().map(|i| *i).unwrap_or(60);
    let entries = inner.sync.entries.lock().ok()?;
    let empty = Entry::default();
    let entry = entries.get(account).unwrap_or(&empty);
    let protocol = match &info.settings {
        None => "demo",
        Some(settings) if settings.protocol == Protocol::Jmap => "jmap",
        Some(_) => "imap",
    };
    let (mode, error) = mode(protocol, info.connected, entry);
    Some(AccountSync {
        account: account.to_owned(),
        protocol: protocol.to_owned(),
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
    inner: &ServiceInner,
    account: &str,
    folder: &str,
    search: &str,
    offset: usize,
    origin: Origin,
) -> Result<Option<Vec<Message>>, Error> {
    let store = inner
        .accounts
        .lock()
        .map_err(|_| Error::Missing)?
        .store(account)?;
    let job = {
        let mut guard = store.lock().map_err(|_| Error::Missing)?;
        match guard.begin_query(folder, search, offset)? {
            Some(job) => job,
            None => return guard.query_page(folder, search, offset).map(Some),
        }
    };
    // Only plain folder refreshes count as syncing; a search or an older page is a lookup.
    let tracked = job.search().is_empty() && offset == 0;
    if tracked {
        let started = inner
            .sync
            .refreshing
            .lock()
            .map_err(|_| Error::Missing)?
            .insert(account.to_owned());
        if !started && origin == Origin::Background {
            return Ok(None);
        }
        inner.sync.update(inner, account, |entry| {
            entry.activity = Some(Activity::Syncing);
            entry.progress = None;
        });
    }
    let result = job
        .run(&mut |done, total| {
            if tracked && total > 0 {
                inner
                    .sync
                    .update(inner, account, |entry| entry.progress = Some((done, total)));
            }
        })
        .and_then(|listing| {
            store
                .lock()
                .map_err(|_| Error::Missing)?
                .finish_query(job, listing)
        });
    if tracked {
        if let Ok(mut refreshing) = inner.sync.refreshing.lock() {
            refreshing.remove(account);
        }
        inner.sync.update(inner, account, |entry| {
            entry.activity = None;
            entry.progress = None;
            match &result {
                Ok(_) => {
                    entry.error = None;
                    if folder == "inbox" {
                        entry.updated_at = Some(crate::api_keys::now());
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
pub(crate) fn start_prefetch(inner: &Arc<ServiceInner>, account: &str) {
    let preferences = match inner.preferences.lock() {
        Ok(preferences) => preferences.get().prefetch.clone(),
        Err(_) => return,
    };
    if !preferences.enabled
        || preferences.count == 0
        || !eligible(inner).iter().any(|a| a == account)
    {
        return;
    }
    if !inner
        .sync
        .prefetching
        .lock()
        .is_ok_and(|mut running| running.insert(account.to_owned()))
    {
        return;
    }
    let (inner, account) = (inner.clone(), account.to_owned());
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        return;
    };
    runtime.spawn(async move {
        if let Ok(_slot) = inner.sync.prefetch_slot.acquire().await {
            let inner_worker = inner.clone();
            let id = account.clone();
            let count = usize::from(preferences.count);
            let _ = tokio::task::spawn_blocking(move || prefetch(&inner_worker, &id, count)).await;
        }
        if let Ok(mut running) = inner.sync.prefetching.lock() {
            running.remove(&account);
        }
    });
}

fn prefetch(inner: &ServiceInner, account: &str, count: usize) {
    let Some((reader, ids)) = (|| {
        let accounts = inner.accounts.lock().ok()?;
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
        inner.sync.update(inner, account, |entry| {
            entry.activity = Some(Activity::Prefetching);
            entry.progress = Some((done, total));
        });
    });
    inner.sync.update(inner, account, |entry| {
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

    fn inner(dir: &std::path::Path) -> Arc<ServiceInner> {
        crate::test_service(dir).inner
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
        let inner = inner(dir.path());
        inner.sync.set_interval(Duration::from_secs(90));
        let mut events = inner.events.subscribe();
        let next = |events: &mut tokio::sync::broadcast::Receiver<ServiceEvent>| {
            let ServiceEvent::Sync(status) = events.try_recv().ok()? else {
                return None;
            };
            Some(status)
        };
        inner.sync.update(&inner, "demo", |entry| {
            entry.activity = Some(Activity::Syncing)
        });
        let event = next(&mut events).unwrap();
        assert_eq!(event.activity, Activity::Syncing);
        assert_eq!(event.mode, Mode::Local);
        assert_eq!(event.interval, 90);
        for done in 1..5 {
            inner
                .sync
                .update(&inner, "demo", |entry| entry.progress = Some((done, 10)));
        }
        // Counts that follow the start faster than the throttle allows are dropped.
        assert!(next(&mut events).is_none());
        inner
            .sync
            .update(&inner, "demo", |entry| entry.progress = Some((10, 10)));
        assert_eq!(next(&mut events).unwrap().progress, Some((10, 10)));
        // Nothing changed, nothing sent.
        inner
            .sync
            .update(&inner, "demo", |entry| entry.progress = Some((10, 10)));
        assert!(next(&mut events).is_none());
        let all = statuses(&inner);
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].activity, Activity::Syncing);
    }

    #[tokio::test]
    async fn demo_refresh_needs_no_provider_and_prefetch_skips_it() {
        let dir = tempfile::tempdir().unwrap();
        let inner = inner(dir.path());
        let messages = refresh(&inner, "demo", "inbox", "", 0, Origin::Owner)
            .unwrap()
            .unwrap();
        assert_eq!(messages.len(), 4);
        // The demo account is never connected, so background work leaves it alone.
        assert!(eligible(&inner).is_empty());
        start_prefetch(&inner, "demo");
        assert!(inner.sync.prefetching.lock().unwrap().is_empty());
    }
}
