//! Reusable, transport-agnostic mail service shared by the daemon, MCP server, and FFI.
pub mod ai;
pub mod api_keys;
pub mod approvals;
pub mod discovery;
pub mod error;
pub mod events;
pub mod logging;
pub mod notifications;
pub mod preferences;
pub mod releases;
pub mod secrets;
pub mod settings;
pub mod sync;

mod store;

use api_keys::{ApiKeys, Principal};
use colecto_core::{
    AccountInfo, AccountInput, Accounts, AttachmentInfo, Capability, Database, Draft, DraftInput,
    Error, Folder, LabelInput, MailReader, MailSnapshot, MailStore, Message, MessageContent,
    NAVIGATION_TARGETS, SecurityInfo, SecurityInput,
    threading::{ThreadMessage, conversation_for},
};
use error::ServiceError;
use events::ServiceEvent;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

/// This build's version: the workspace `version` in the root `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The Git commit this build came from, when the build sets `COLECTO_COMMIT`.
pub fn commit() -> Option<&'static str> {
    option_env!("COLECTO_COMMIT").filter(|commit| !commit.is_empty())
}

#[derive(Clone, Debug, Default, Deserialize)]
pub struct MessageQuery {
    #[serde(default = "inbox_folder")]
    pub folder: String,
    #[serde(default)]
    pub q: String,
    #[serde(default)]
    pub offset: usize,
}

fn inbox_folder() -> String {
    "inbox".into()
}

#[derive(Clone, Copy, Debug, Default, Deserialize)]
pub struct ContentOptions {
    #[serde(default)]
    pub cached: bool,
    pub remote_images: Option<bool>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OrganizeInput {
    pub ids: Vec<String>,
    pub seen: Option<bool>,
    pub flagged: Option<bool>,
    pub folder: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendApprovalReceipt {
    pub id: String,
    pub status: approvals::Status,
    pub expires_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DeliveryOutcome {
    Sent(bool),
    ApprovalRequired(SendApprovalReceipt),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendStatus {
    pub id: String,
    pub status: approvals::Status,
    pub expires_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavigateResult {
    pub target: String,
    pub clients: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseStatus {
    pub current: String,
    pub latest: Option<releases::Release>,
}

pub struct WindowGuard(Arc<AtomicUsize>);

impl Drop for WindowGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

pub(crate) struct ServiceInner {
    pub(crate) events: tokio::sync::broadcast::Sender<ServiceEvent>,
    pub(crate) keys: Arc<Mutex<ApiKeys>>,
    pub(crate) preferences: Arc<Mutex<preferences::PreferenceStore>>,
    pub(crate) approvals: Arc<Mutex<approvals::Approvals>>,
    pub(crate) status_polls: Arc<Mutex<HashMap<String, Instant>>>,
    pub(crate) accounts: Arc<Mutex<Accounts>>,
    pub(crate) execution: Arc<tokio::sync::Semaphore>,
    pub(crate) ai: Arc<ai::Ai>,
    pub(crate) sync: Arc<sync::SyncState>,
    pub(crate) windows: Arc<AtomicUsize>,
    snapshots: Arc<Mutex<NotificationSnapshots>>,
    watching: Arc<Mutex<Watching>>,
    closing: tokio::sync::watch::Sender<bool>,
}

#[derive(Clone)]
pub struct MailService {
    inner: Arc<ServiceInner>,
}

impl MailService {
    pub fn new(accounts: Accounts) -> Result<Self, String> {
        let database = accounts.database();
        let keys = Arc::new(Mutex::new(ApiKeys::open(database.clone())?));
        let preferences = Arc::new(Mutex::new(preferences::PreferenceStore::open(
            database.clone(),
        )?));
        let ai = Arc::new(ai::Ai::open(database.clone())?);
        let approvals = Arc::new(Mutex::new(approvals::Approvals::open(
            database,
            api_keys::now(),
        )?));
        let (events, _) = tokio::sync::broadcast::channel(64);
        let (closing, _) = tokio::sync::watch::channel(false);
        Ok(Self {
            inner: Arc::new(ServiceInner {
                events,
                keys,
                preferences,
                approvals,
                status_polls: Arc::default(),
                accounts: Arc::new(Mutex::new(accounts)),
                execution: Arc::new(tokio::sync::Semaphore::new(2)),
                ai,
                sync: Arc::default(),
                windows: Arc::default(),
                snapshots: Arc::default(),
                watching: Arc::default(),
                closing,
            }),
        })
    }

    /// The shared application database, for callers that own this service's lifecycle.
    pub fn database(&self) -> Database {
        self.inner
            .accounts
            .lock()
            .map_or_else(|e| e.into_inner().database(), |a| a.database())
    }

    pub fn data_directory(&self) -> PathBuf {
        self.inner.accounts.lock().map_or_else(
            |e| e.into_inner().data_directory().to_path_buf(),
            |a| a.data_directory().to_path_buf(),
        )
    }

    pub fn mcp_enabled(&self) -> bool {
        self.inner.preferences.lock().is_ok_and(|p| p.get().mcp)
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<ServiceEvent> {
        self.inner.events.subscribe()
    }

    pub fn closing_receiver(&self) -> tokio::sync::watch::Receiver<bool> {
        self.inner.closing.subscribe()
    }

    pub fn open_window(&self) -> WindowGuard {
        self.inner.windows.fetch_add(1, Ordering::Relaxed);
        WindowGuard(self.inner.windows.clone())
    }

    pub fn open_windows(&self) -> usize {
        self.inner.windows.load(Ordering::Relaxed)
    }

    pub fn close(&self) {
        let _ = self.inner.closing.send(true);
        let _ = self.inner.events.send(ServiceEvent::Shutdown);
    }

    pub fn authenticate_api_key(&self, secret: &str) -> Result<Principal, ServiceError> {
        self.inner
            .keys
            .lock()
            .map_err(|_| ServiceError::forbidden())?
            .authenticate(secret, api_keys::now())
            .map_err(ServiceError::unauthorized)
    }

    pub fn try_acquire_execution(&self) -> Result<tokio::sync::OwnedSemaphorePermit, ServiceError> {
        self.inner
            .execution
            .clone()
            .try_acquire_owned()
            .map_err(|_| ServiceError::too_many_requests("Mail service busy; try again"))
    }

    pub async fn account_connected(
        &self,
        principal: &Principal,
        account: &str,
    ) -> Result<bool, ServiceError> {
        authorize_inner(&self.inner, principal, account, Capability::AccountRead)?;
        with_store(
            self.inner.clone(),
            account.to_owned(),
            |s| Ok(s.connected()),
        )
        .await
    }

    pub async fn folders(
        &self,
        principal: &Principal,
        account: &str,
    ) -> Result<Vec<Folder>, ServiceError> {
        authorize_inner(&self.inner, principal, account, Capability::MessagesList)?;
        with_store(self.inner.clone(), account.to_owned(), |s| s.folders()).await
    }

    pub async fn labels(
        &self,
        principal: &Principal,
        account: &str,
    ) -> Result<BTreeMap<String, Vec<String>>, ServiceError> {
        authorize_inner(&self.inner, principal, account, Capability::MessagesList)?;
        with_store(self.inner.clone(), account.to_owned(), |s| s.labels()).await
    }

    pub async fn update_label(
        &self,
        principal: &Principal,
        account: &str,
        input: LabelInput,
    ) -> Result<BTreeMap<String, Vec<String>>, ServiceError> {
        authorize_inner(&self.inner, principal, account, Capability::MessagesLabel)?;
        with_store(self.inner.clone(), account.to_owned(), move |s| {
            s.update_label(&input.name, input.message_id.as_deref(), input.remove)
        })
        .await
    }

    pub async fn cached_messages(
        &self,
        principal: &Principal,
        account: &str,
        query: MessageQuery,
    ) -> Result<MailSnapshot, ServiceError> {
        authorize_inner(&self.inner, principal, account, Capability::MessagesList)?;
        if !query.q.is_empty() {
            return Err(ServiceError::bad_request(
                "Cached snapshots do not run provider searches",
            ));
        }
        let mut snapshot = self
            .inner
            .accounts
            .lock()
            .map_err(|_| ServiceError::forbidden())?
            .cached(account, &query.folder)
            .map_err(ServiceError::from)?;
        redact_list_bodies(&self.inner, principal, account, &mut snapshot.messages)?;
        Ok(snapshot)
    }

    pub async fn messages(
        &self,
        principal: &Principal,
        account: &str,
        query: MessageQuery,
    ) -> Result<Vec<Message>, ServiceError> {
        authorize_inner(&self.inner, principal, account, Capability::MessagesList)?;
        self.inner
            .accounts
            .lock()
            .map_err(|_| ServiceError::forbidden())?
            .store(account)
            .map_err(|_| ServiceError::forbidden())?;
        let (inner, id) = (self.inner.clone(), account.to_owned());
        let mut messages = tokio::task::spawn_blocking(move || {
            let messages = sync::refresh(
                &inner,
                &id,
                &query.folder,
                &query.q,
                query.offset,
                sync::Origin::Owner,
            )?
            .unwrap_or_default();
            if query.folder == "inbox" && query.q.trim().is_empty() && query.offset == 0 {
                sync::start_prefetch(&inner, &id);
            }
            Ok::<_, Error>(messages)
        })
        .await
        .map_err(|_| ServiceError::internal("Mail service unavailable"))??;
        redact_list_bodies(&self.inner, principal, account, &mut messages)?;
        Ok(messages)
    }

    pub async fn organize_messages(
        &self,
        principal: &Principal,
        account: &str,
        action: &str,
        input: OrganizeInput,
    ) -> Result<(), ServiceError> {
        authorize_inner(
            &self.inner,
            principal,
            account,
            Capability::MessagesOrganize,
        )?;
        if input.ids.is_empty() || input.ids.len() > 500 {
            return Err(ServiceError::bad_request(
                "Supply between 1 and 500 message ids",
            ));
        }
        let action = action.to_owned();
        with_store(
            self.inner.clone(),
            account.to_owned(),
            move |store| match action.as_str() {
                "flags" => store.set_flags(&input.ids, input.seen, input.flagged),
                "move" => store.move_to(
                    &input.ids,
                    input
                        .folder
                        .as_deref()
                        .ok_or_else(|| Error::Invalid("Folder required".into()))?,
                ),
                "archive" => store.archive(&input.ids),
                "delete" => store.delete(&input.ids),
                _ => Err(Error::Invalid("Unknown operation".into())),
            },
        )
        .await?;
        let _ = self.inner.events.send(ServiceEvent::Mail {
            account: account.to_owned(),
            ids: Vec::new(),
        });
        Ok(())
    }

    pub async fn message(
        &self,
        principal: &Principal,
        account: &str,
        id: &str,
    ) -> Result<Message, ServiceError> {
        authorize_inner(&self.inner, principal, account, Capability::MessagesRead)?;
        if let Some(message) = self
            .inner
            .accounts
            .lock()
            .map_err(|_| ServiceError::forbidden())?
            .cached_message(account, id)
            .map_err(ServiceError::from)?
        {
            return Ok(message);
        }
        let id = id.to_owned();
        with_store(self.inner.clone(), account.to_owned(), move |s| {
            s.message(&id).cloned().ok_or(Error::Missing)
        })
        .await
    }

    pub async fn conversation(
        &self,
        principal: &Principal,
        account: &str,
        id: &str,
    ) -> Result<Vec<ThreadMessage>, ServiceError> {
        authorize_inner(&self.inner, principal, account, Capability::MessagesRead)?;
        let snapshots = self
            .inner
            .accounts
            .lock()
            .map_err(|_| ServiceError::forbidden())?
            .all_cached(account)
            .map_err(|_| ServiceError::forbidden())?;
        let current = snapshots.iter().find_map(|(folder, snapshot)| {
            snapshot
                .messages
                .iter()
                .find(|m| m.id == id)
                .map(|m| (folder.clone(), m.clone()))
        });
        let (current_folder, current_msg) = match current {
            Some(found) => found,
            None => {
                let id_owned = id.to_owned();
                let msg = with_store(self.inner.clone(), account.to_owned(), move |s| {
                    s.message(&id_owned).cloned().ok_or(Error::Missing)
                })
                .await?;
                (String::new(), msg)
            }
        };
        let other_folders: Vec<(&str, &[Message])> = snapshots
            .iter()
            .map(|(folder, snapshot)| (folder.as_str(), snapshot.messages.as_slice()))
            .collect();
        Ok(conversation_for(
            &current_folder,
            &current_msg,
            &other_folders,
            20,
        ))
    }

    pub async fn message_content(
        &self,
        principal: &Principal,
        account: &str,
        id: &str,
        options: ContentOptions,
    ) -> Result<MessageContent, ServiceError> {
        authorize_inner(&self.inner, principal, account, Capability::MessagesRead)?;
        let default_remote = self
            .inner
            .preferences
            .lock()
            .map_err(|_| ServiceError::forbidden())?
            .get()
            .remote_images;
        let allow_remote_images = options.remote_images.unwrap_or(default_remote);
        let id_demo = id.to_owned();
        let id_reader = id.to_owned();
        with_reader(
            self.inner.clone(),
            account.to_owned(),
            move |store| store.content_with_options(&id_demo, allow_remote_images),
            move |reader| {
                if options.cached && !reader.is_cached(&id_reader) {
                    return Err(Error::Missing);
                }
                reader.content_with_options(&id_reader, allow_remote_images)
            },
        )
        .await
    }

    pub async fn message_attachment(
        &self,
        principal: &Principal,
        account: &str,
        id: &str,
        index: usize,
    ) -> Result<(AttachmentInfo, Vec<u8>), ServiceError> {
        authorize_inner(&self.inner, principal, account, Capability::MessagesRead)?;
        let id_demo = id.to_owned();
        let id_reader = id.to_owned();
        with_reader(
            self.inner.clone(),
            account.to_owned(),
            move |store| store.attachment(&id_demo, index),
            move |reader| reader.attachment(&id_reader, index),
        )
        .await
    }

    pub async fn drafts(
        &self,
        principal: &Principal,
        account: &str,
    ) -> Result<Vec<Draft>, ServiceError> {
        authorize_inner(&self.inner, principal, account, Capability::DraftsList)?;
        let mut drafts = with_store(self.inner.clone(), account.to_owned(), |s| {
            Ok(s.drafts().to_vec())
        })
        .await?;
        if let Principal::ApiKey(id) = principal
            && !self
                .inner
                .keys
                .lock()
                .map_err(|_| ServiceError::forbidden())?
                .allows(id, account, Capability::DraftsRead, api_keys::now())
        {
            for draft in &mut drafts {
                draft.body.clear();
            }
        }
        Ok(drafts)
    }

    pub async fn draft(
        &self,
        principal: &Principal,
        account: &str,
        id: &str,
    ) -> Result<Draft, ServiceError> {
        authorize_inner(&self.inner, principal, account, Capability::DraftsRead)?;
        let id = id.to_owned();
        with_store(self.inner.clone(), account.to_owned(), move |s| {
            s.draft(&id).cloned().ok_or(Error::Missing)
        })
        .await
    }

    pub async fn save_draft(
        &self,
        principal: &Principal,
        account: &str,
        input: DraftInput,
    ) -> Result<Draft, ServiceError> {
        authorize_inner(&self.inner, principal, account, Capability::DraftsWrite)?;
        with_store(self.inner.clone(), account.to_owned(), move |s| {
            s.save_draft(input)
        })
        .await
    }

    pub async fn send(
        &self,
        principal: &Principal,
        account: &str,
        input: DraftInput,
    ) -> Result<DeliveryOutcome, ServiceError> {
        authorize_inner(&self.inner, principal, account, Capability::MessagesSend)?;
        if input.forward_of.is_some() {
            authorize_inner(&self.inner, principal, account, Capability::MessagesRead)?;
        }
        if let Principal::ApiKey(id) = principal {
            let receipt =
                request_delivery(self.inner.clone(), id.clone(), account.to_owned(), input).await?;
            return Ok(DeliveryOutcome::ApprovalRequired(receipt));
        }
        with_store(self.inner.clone(), account.to_owned(), move |s| {
            s.send(input)?;
            Ok(DeliveryOutcome::Sent(true))
        })
        .await
    }

    pub async fn list_accounts(
        &self,
        principal: &Principal,
    ) -> Result<Vec<AccountInfo>, ServiceError> {
        owner_only(principal)?;
        with_accounts(self.inner.clone(), |a| Ok(a.list())).await
    }

    pub async fn upsert_account(
        &self,
        principal: &Principal,
        input: AccountInput,
    ) -> Result<AccountInfo, ServiceError> {
        owner_only(principal)?;
        with_accounts(self.inner.clone(), move |a| a.upsert(input)).await
    }

    pub async fn discover_account(
        &self,
        principal: &Principal,
        address: &str,
    ) -> Result<Option<discovery::Discovery>, ServiceError> {
        owner_only(principal)?;
        if address.len() > 320 {
            return Err(ServiceError::bad_request("Invalid email address"));
        }
        let found = tokio::time::timeout(discovery::DEADLINE, discovery::discover(address))
            .await
            .ok()
            .flatten();
        Ok(found)
    }

    pub async fn remove_account(
        &self,
        principal: &Principal,
        id: &str,
    ) -> Result<bool, ServiceError> {
        owner_only(principal)?;
        let inner = self.inner.clone();
        let id = id.to_owned();
        tokio::task::spawn_blocking(move || {
            inner
                .accounts
                .lock()
                .map_err(|_| ServiceError::forbidden())?
                .remove(&id)
                .map_err(ServiceError::from)?;
            inner.ai.remove_account(&id);
            inner
                .preferences
                .lock()
                .map_err(|_| ServiceError::forbidden())?
                .clear_account(&id)
                .map_err(ServiceError::bad_request)?;
            Ok(true)
        })
        .await
        .map_err(|_| ServiceError::forbidden())?
    }

    pub async fn disconnect_account(
        &self,
        principal: &Principal,
        id: &str,
    ) -> Result<bool, ServiceError> {
        owner_only(principal)?;
        let id = id.to_owned();
        with_accounts(self.inner.clone(), move |a| {
            a.disconnect(&id)?;
            Ok(true)
        })
        .await
    }

    pub async fn security(&self, principal: &Principal) -> Result<SecurityInfo, ServiceError> {
        owner_only(principal)?;
        with_accounts(self.inner.clone(), |a| Ok(a.security())).await
    }

    pub async fn security_action(
        &self,
        principal: &Principal,
        mut input: SecurityInput,
    ) -> Result<SecurityInfo, ServiceError> {
        owner_only(principal)?;
        with_accounts(self.inner.clone(), move |a| {
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
            result?;
            Ok(a.security())
        })
        .await
    }

    pub fn get_preferences(
        &self,
        principal: &Principal,
    ) -> Result<preferences::PreferencesView, ServiceError> {
        owner_only(principal)?;
        Ok(self
            .inner
            .preferences
            .lock()
            .map_err(|_| ServiceError::forbidden())?
            .view())
    }

    pub async fn update_preferences(
        &self,
        principal: &Principal,
        input: preferences::PreferencesInput,
    ) -> Result<preferences::PreferencesView, ServiceError> {
        owner_only(principal)?;
        let inner = self.inner.clone();
        tokio::task::spawn_blocking(move || {
            let mut store = inner
                .preferences
                .lock()
                .map_err(|_| ServiceError::forbidden())?;
            let before = store.get().mcp;
            let saved = store.update(input).map_err(ServiceError::bad_request)?;
            if saved.mcp != before {
                let _ = inner.events.send(ServiceEvent::Mcp { on: saved.mcp });
            }
            Ok(store.view())
        })
        .await
        .map_err(|_| ServiceError::forbidden())?
    }

    pub fn list_api_keys(
        &self,
        principal: &Principal,
    ) -> Result<Vec<api_keys::KeyMetadata>, ServiceError> {
        owner_only(principal)?;
        Ok(self
            .inner
            .keys
            .lock()
            .map_err(|_| ServiceError::forbidden())?
            .list())
    }

    pub fn mint_api_key(
        &self,
        principal: &Principal,
        input: api_keys::KeyInput,
    ) -> Result<(api_keys::KeyMetadata, String), ServiceError> {
        owner_only(principal)?;
        let accounts = self
            .inner
            .accounts
            .lock()
            .map_err(|_| ServiceError::forbidden())?
            .list();
        if input
            .accounts
            .iter()
            .any(|id| !accounts.iter().any(|a| &a.id == id))
        {
            return Err(ServiceError::forbidden());
        }
        self.inner
            .keys
            .lock()
            .map_err(|_| ServiceError::forbidden())?
            .mint(input, api_keys::now())
            .map_err(ServiceError::bad_request)
    }

    pub fn revoke_api_key(&self, principal: &Principal, id: &str) -> Result<bool, ServiceError> {
        owner_only(principal)?;
        self.inner
            .keys
            .lock()
            .map_err(|_| ServiceError::forbidden())?
            .revoke(id, api_keys::now())
            .map_err(ServiceError::bad_request)?;
        Ok(true)
    }

    pub fn list_send_requests(
        &self,
        principal: &Principal,
    ) -> Result<Vec<approvals::PendingSend>, ServiceError> {
        owner_only(principal)?;
        self.inner
            .approvals
            .lock()
            .map_err(|_| ServiceError::forbidden())?
            .list(api_keys::now())
            .map_err(ServiceError::conflict)
    }

    pub fn send_request_status(
        &self,
        principal: &Principal,
        id: &str,
        account: Option<&str>,
    ) -> Result<SendStatus, ServiceError> {
        if let Principal::ApiKey(key) = principal {
            let mut polls = self
                .inner
                .status_polls
                .lock()
                .map_err(|_| ServiceError::forbidden())?;
            if polls
                .get(key)
                .is_some_and(|t| t.elapsed() < Duration::from_secs(1))
            {
                return Err(ServiceError::too_many_requests(
                    "Poll at most once per second",
                ));
            }
            if polls.len() >= 128 {
                polls.retain(|_, t| t.elapsed() < Duration::from_secs(1));
            }
            polls.insert(key.clone(), Instant::now());
        }
        let request = self
            .inner
            .approvals
            .lock()
            .map_err(|_| ServiceError::forbidden())?
            .get(id, api_keys::now())
            .map_err(|_| ServiceError::not_found("Delivery request not found"))?;
        if account.is_some_and(|account| request.account != account) {
            return Err(ServiceError::not_found("Delivery request not found"));
        }
        if let Principal::ApiKey(key) = principal
            && (request.requester != *key
                || !self
                    .inner
                    .keys
                    .lock()
                    .map_err(|_| ServiceError::forbidden())?
                    .allows(
                        key,
                        &request.account,
                        Capability::MessagesSend,
                        api_keys::now(),
                    ))
        {
            return Err(ServiceError::not_found("Delivery request not found"));
        }
        Ok(SendStatus {
            id: request.id,
            status: request.status,
            expires_at: request.expires_at,
        })
    }

    pub fn reject_send(
        &self,
        principal: &Principal,
        id: &str,
    ) -> Result<approvals::PendingSend, ServiceError> {
        owner_only(principal)?;
        self.inner
            .approvals
            .lock()
            .map_err(|_| ServiceError::forbidden())?
            .reject(id, api_keys::now())
            .map_err(ServiceError::conflict)
    }

    pub async fn approve_send(
        &self,
        principal: &Principal,
        id: &str,
    ) -> Result<approvals::PendingSend, ServiceError> {
        owner_only(principal)?;
        let permit = self.try_acquire_execution()?;
        let inner = self.inner.clone();
        let id = id.to_owned();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let accounts = inner
                .accounts
                .lock()
                .map_err(|_| ServiceError::forbidden())?;
            let request = inner
                .approvals
                .lock()
                .map_err(|_| ServiceError::forbidden())?
                .get(&id, api_keys::now())
                .map_err(ServiceError::conflict)?;
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
                .ok_or_else(ServiceError::forbidden)?;
            let store = accounts.store(&info.id).map_err(ServiceError::from)?;
            let store = store
                .try_lock()
                .map_err(|_| ServiceError::too_many_requests("Mail service busy; try again"))?;
            let mut keys = inner.keys.lock().map_err(|_| ServiceError::forbidden())?;
            if !keys.allows(
                &request.requester,
                &request.account,
                Capability::MessagesSend,
                api_keys::now(),
            ) {
                return Err(ServiceError::forbidden());
            }
            let snapshot = inner
                .approvals
                .lock()
                .map_err(|_| ServiceError::forbidden())?
                .begin(&id, api_keys::now())
                .map_err(ServiceError::conflict)?;
            drop(keys);
            drop(accounts);
            let sent = store.send(snapshot).is_ok();
            let mut approvals = inner
                .approvals
                .lock()
                .map_err(|_| ServiceError::forbidden())?;
            approvals
                .finish(&id, sent)
                .map_err(ServiceError::conflict)?;
            approvals
                .get(&id, api_keys::now())
                .map_err(ServiceError::conflict)
        })
        .await
        .map_err(|_| ServiceError::conflict("Delivery outcome uncertain; do not retry"))?
    }

    pub fn navigate(
        &self,
        principal: &Principal,
        account: &str,
        target: &str,
    ) -> Result<NavigateResult, ServiceError> {
        if !NAVIGATION_TARGETS.contains(&target) {
            return Err(ServiceError::bad_request("Unknown navigation target"));
        }
        authorize_inner(&self.inner, principal, account, Capability::AppNavigate)?;
        let clients = self
            .inner
            .events
            .send(ServiceEvent::Navigate {
                target: target.to_owned(),
                account: account.to_owned(),
            })
            .unwrap_or(0);
        Ok(NavigateResult {
            target: target.to_owned(),
            clients,
        })
    }

    pub fn read_scoped_settings(
        &self,
        principal: &Principal,
        account: &str,
    ) -> Result<serde_json::Value, ServiceError> {
        settings::read(&self.inner, principal, account)
    }

    pub fn update_scoped_settings(
        &self,
        principal: &Principal,
        account: &str,
        input: settings::SettingsInput,
    ) -> Result<serde_json::Value, ServiceError> {
        settings::update(&self.inner, principal, account, input)
    }

    pub async fn sync_status(
        &self,
        principal: &Principal,
    ) -> Result<Vec<sync::AccountSync>, ServiceError> {
        owner_only(principal)?;
        let inner = self.inner.clone();
        Ok(tokio::task::spawn_blocking(move || sync::statuses(&inner))
            .await
            .unwrap_or_default())
    }

    pub async fn latest_release(
        &self,
        principal: &Principal,
    ) -> Result<ReleaseStatus, ServiceError> {
        if let Principal::ApiKey(_) = principal {
            return Err(ServiceError::forbidden());
        }
        let latest = tokio::task::spawn_blocking(|| releases::latest(VERSION))
            .await
            .map_err(|_| ServiceError::forbidden())?
            .map_err(ServiceError::bad_gateway)?;
        Ok(ReleaseStatus {
            current: VERSION.to_owned(),
            latest,
        })
    }

    pub fn ai_settings(
        &self,
        principal: &Principal,
        account: Option<&str>,
    ) -> Result<ai::SettingsView, ServiceError> {
        ai::get_settings(&self.inner, principal, account)
    }

    pub async fn ai_update_settings(
        &self,
        principal: &Principal,
        account: Option<&str>,
        input: ai::AiSettings,
    ) -> Result<ai::SettingsView, ServiceError> {
        ai::put_settings(self.inner.clone(), principal, account, input).await
    }

    pub async fn ai_put_key(
        &self,
        principal: &Principal,
        input: ai::KeyInput,
    ) -> Result<ai::SettingsView, ServiceError> {
        ai::put_key(self.inner.clone(), principal, input).await
    }

    pub async fn ai_test_connection(
        &self,
        principal: &Principal,
        account: Option<&str>,
        input: ai::TestInput,
    ) -> Result<bool, ServiceError> {
        ai::test_connection(&self.inner, principal, account, input).await
    }

    pub fn ai_preview(
        &self,
        principal: &Principal,
        account: &str,
        input: ai::PreviewInput,
    ) -> Result<ai::PreviewResult, ServiceError> {
        ai::preview(&self.inner, principal, account, input)
    }

    pub async fn ai_reclassify(
        &self,
        principal: &Principal,
        account: &str,
        input: ai::ReclassifyInput,
    ) -> Result<ai::Reclassified, ServiceError> {
        ai::reclassify(&self.inner, principal, account, input).await
    }

    pub async fn ai_get_summary(
        &self,
        principal: &Principal,
        account: &str,
        id: &str,
    ) -> Result<Option<String>, ServiceError> {
        ai::get_summary(self.inner.clone(), principal, account, id).await
    }

    pub async fn ai_create_summary(
        &self,
        principal: &Principal,
        account: &str,
        id: &str,
    ) -> Result<String, ServiceError> {
        ai::create_summary(self.inner.clone(), principal, account, id).await
    }

    pub async fn ai_delete_summary(
        &self,
        principal: &Principal,
        account: &str,
        id: &str,
    ) -> Result<(), ServiceError> {
        ai::delete_summary(self.inner.clone(), principal, account, id).await
    }

    pub async fn ai_draft_reply(
        &self,
        principal: &Principal,
        account: &str,
        input: ai::ReplyInput,
    ) -> Result<String, ServiceError> {
        ai::draft_reply(&self.inner, principal, account, input).await
    }

    /// Connected accounts the owner left on: one bounded polling pass.
    pub async fn refresh_once(&self) {
        let (_stop, receiver) = tokio::sync::watch::channel(false);
        self.refresh_until_stopped(receiver, None).await;
    }

    async fn refresh_until_stopped(
        &self,
        stop: tokio::sync::watch::Receiver<bool>,
        only: Option<&str>,
    ) {
        if let Ok(mut approvals) = self.inner.approvals.lock()
            && approvals.maintain(api_keys::now()).is_err()
        {
            tracing::warn!("Could not expire retained delivery requests");
        }
        let accounts = sync::eligible(&self.inner)
            .into_iter()
            .filter(|id| only.is_none_or(|only| only == id));
        let mut tasks = tokio::task::JoinSet::new();
        for account in accounts {
            let inner = self.inner.clone();
            let stop = stop.clone();
            tasks.spawn(async move {
                let Some(_permit) =
                    acquire_for_refresh(inner.execution.clone(), stop.clone()).await
                else {
                    return;
                };
                let inner_work = inner.clone();
                let account_work = account.clone();
                let result = tokio::task::spawn_blocking(move || {
                    if *stop.borrow() {
                        return None;
                    }
                    let refreshed = match sync::refresh(
                        &inner_work,
                        &account_work,
                        "inbox",
                        "",
                        0,
                        sync::Origin::Background,
                    ) {
                        Ok(None) => return None,
                        Ok(Some(messages)) => Ok(messages),
                        Err(error) => Err(error),
                    };
                    if let Ok(mut watching) = inner_work.watching.lock() {
                        if refreshed.is_ok() {
                            watching.healthy.insert(account_work.clone());
                        } else {
                            watching.healthy.remove(&account_work);
                        }
                    }
                    match refreshed {
                        Ok(messages) => {
                            tracing::debug!(
                                account = %account_work,
                                count = messages.len(),
                                "Refreshed mail"
                            );
                            sync::start_prefetch(&inner_work, &account_work);
                            if let Ok(mut snapshots) = inner_work.snapshots.lock()
                                && let Some(new_ids) = snapshots
                                    .observe(&account_work, messages.iter().map(|m| m.id.clone()))
                            {
                                let _ = inner_work.events.send(ServiceEvent::Mail {
                                    account: account_work,
                                    ids: new_ids.clone(),
                                });
                                return Some(new_ids);
                            }
                            None
                        }
                        Err(error) => {
                            tracing::warn!(
                                account = %account_work,
                                %error,
                                "Background receive failed"
                            );
                            None
                        }
                    }
                })
                .await;
                match result {
                    Ok(Some(new_ids)) => {
                        ai::auto_label_new_mail(&inner, &account, &new_ids).await;
                    }
                    Ok(None) => {}
                    Err(error) => tracing::error!(%error, "Background mail task failed"),
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
        self.inner.sync.set_interval(interval);
        loop {
            if *stop.borrow() {
                break;
            }
            self.refresh_until_stopped(stop.clone(), None).await;
            self.watch_push(&stop);
            tokio::select! {
                _ = tokio::time::sleep(interval) => {},
                result = stop.changed() => {
                    if result.is_err() || *stop.borrow() {
                        break;
                    }
                }
            }
        }
        self.close();
    }

    fn watch_push(&self, stop: &tokio::sync::watch::Receiver<bool>) {
        let candidates = {
            let eligible = sync::eligible(&self.inner);
            let Ok(accounts) = self.inner.accounts.lock() else {
                return;
            };
            eligible
                .into_iter()
                .filter_map(|id| accounts.store(&id).ok().map(|s| (id, s)))
                .collect::<Vec<_>>()
        };
        let healthy = self
            .inner
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
                let Ok(mut watching) = self.inner.watching.lock() else {
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
            std::thread::spawn(move || {
                service.inner.sync.set_push(&service.inner, &account, true);
                while !*stop.borrow() {
                    match watcher.wait(PUSH_WAIT) {
                        Ok(true) => runtime
                            .block_on(service.refresh_until_stopped(stop.clone(), Some(&account))),
                        Ok(false) => {}
                        Err(Error::Invalid(_)) => {
                            if let Ok(mut watching) = service.inner.watching.lock() {
                                watching.unsupported.insert(watcher.key().to_owned());
                            }
                            break;
                        }
                        Err(error) => {
                            tracing::warn!(%account, %error, "Push notifications paused");
                            break;
                        }
                    }
                    let current = service
                        .inner
                        .accounts
                        .lock()
                        .ok()
                        .and_then(|accounts| accounts.store(&account).ok())
                        .and_then(|store| store.lock().ok().and_then(|s| s.push_watcher()));
                    let enabled = service
                        .inner
                        .preferences
                        .lock()
                        .is_ok_and(|p| p.get().account_enabled(&account));
                    if !enabled || current.is_none_or(|current| current.key() != watcher.key()) {
                        break;
                    }
                }
                if let Ok(mut watching) = service.inner.watching.lock() {
                    watching.running.remove(&account);
                }
                service.inner.sync.set_push(&service.inner, &account, false);
            });
        }
    }
}

pub(crate) fn owner_only(principal: &Principal) -> Result<(), ServiceError> {
    if !matches!(principal, Principal::Owner) {
        return Err(ServiceError::forbidden());
    }
    Ok(())
}

pub(crate) fn authorize_inner(
    inner: &ServiceInner,
    principal: &Principal,
    account: &str,
    operation: Capability,
) -> Result<(), ServiceError> {
    if account.is_empty() {
        return Err(ServiceError::forbidden());
    }
    match principal {
        Principal::Owner => Ok(()),
        Principal::ApiKey(id) => {
            if inner
                .keys
                .lock()
                .map_err(|_| ServiceError::forbidden())?
                .allows(id, account, operation, api_keys::now())
            {
                Ok(())
            } else {
                Err(ServiceError::forbidden())
            }
        }
    }
}

fn redact_list_bodies(
    inner: &ServiceInner,
    principal: &Principal,
    account: &str,
    messages: &mut [Message],
) -> Result<(), ServiceError> {
    if let Principal::ApiKey(id) = principal
        && !inner
            .keys
            .lock()
            .map_err(|_| ServiceError::forbidden())?
            .allows(id, account, Capability::MessagesRead, api_keys::now())
    {
        for message in messages {
            message.body.clear();
        }
    }
    Ok(())
}

pub(crate) async fn with_accounts<T: Send + 'static>(
    inner: Arc<ServiceInner>,
    action: impl FnOnce(&mut Accounts) -> Result<T, Error> + Send + 'static,
) -> Result<T, ServiceError> {
    tokio::task::spawn_blocking(move || {
        let mut accounts = inner
            .accounts
            .lock()
            .map_err(|_| ServiceError::forbidden())?;
        action(&mut accounts).map_err(ServiceError::from)
    })
    .await
    .map_err(|_| ServiceError::internal("Account service unavailable"))?
}

pub(crate) async fn with_store<T: Send + 'static>(
    inner: Arc<ServiceInner>,
    account: String,
    action: impl FnOnce(&mut MailStore) -> Result<T, Error> + Send + 'static,
) -> Result<T, ServiceError> {
    tokio::task::spawn_blocking(move || {
        let handle = inner
            .accounts
            .lock()
            .map_err(|_| ServiceError::forbidden())?
            .store(&account)
            .map_err(|_| ServiceError::forbidden())?;
        let mut store = handle
            .lock()
            .map_err(|_| ServiceError::internal("Mail store unavailable"))?;
        action(&mut store).map_err(ServiceError::from)
    })
    .await
    .map_err(|_| ServiceError::internal("Mail service unavailable"))?
}

async fn with_reader<T: Send + 'static>(
    inner: Arc<ServiceInner>,
    account: String,
    demo: impl FnOnce(&mut MailStore) -> Result<T, Error> + Send + 'static,
    action: impl FnOnce(&MailReader) -> Result<T, Error> + Send + 'static,
) -> Result<T, ServiceError> {
    tokio::task::spawn_blocking(move || {
        let handle = inner
            .accounts
            .lock()
            .map_err(|_| ServiceError::forbidden())?
            .store(&account)
            .map_err(|_| ServiceError::forbidden())?;
        let unavailable = || ServiceError::internal("Mail store unavailable");
        let reader = handle.lock().map_err(|_| unavailable())?.reader();
        match reader {
            Some(reader) => action(&reader).map_err(ServiceError::from),
            None => {
                demo(&mut *handle.lock().map_err(|_| unavailable())?).map_err(ServiceError::from)
            }
        }
    })
    .await
    .map_err(|_| ServiceError::internal("Mail service unavailable"))?
}

async fn request_delivery(
    inner: Arc<ServiceInner>,
    key: String,
    account: String,
    input: DraftInput,
) -> Result<SendApprovalReceipt, ServiceError> {
    tokio::task::spawn_blocking(move || {
        let accounts = inner
            .accounts
            .lock()
            .map_err(|_| ServiceError::forbidden())?;
        let info = accounts
            .list()
            .into_iter()
            .find(|a| a.id == account && a.connected)
            .ok_or_else(ServiceError::forbidden)?;
        let from = info.settings.ok_or_else(ServiceError::forbidden)?.address;
        let mut keys = inner.keys.lock().map_err(|_| ServiceError::forbidden())?;
        if !keys.allows(&key, &account, Capability::MessagesSend, api_keys::now()) {
            return Err(ServiceError::forbidden());
        }
        let subject = input.subject.clone();
        let request = inner
            .approvals
            .lock()
            .map_err(|_| ServiceError::forbidden())?
            .create(&key, &account, &from, input, api_keys::now())
            .map_err(ServiceError::conflict)?;
        let _ = inner.events.send(ServiceEvent::Approval {
            id: request.id.clone(),
            account,
            from,
            subject,
        });
        Ok(SendApprovalReceipt {
            id: request.id,
            status: request.status,
            expires_at: request.expires_at,
        })
    })
    .await
    .map_err(|_| ServiceError::conflict("Delivery request service unavailable"))?
}

const PUSH_WAIT: Duration = Duration::from_secs(10 * 60);

#[derive(Default)]
struct Watching {
    healthy: HashSet<String>,
    running: HashSet<String>,
    unsupported: HashSet<String>,
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
                if changed.is_err() || *stop.borrow() {
                    return None;
                }
            }
            permit = semaphore.clone().acquire_owned() => return permit.ok(),
        }
    }
}

#[derive(Default)]
struct NotificationSnapshots(HashMap<String, HashSet<String>>);

impl NotificationSnapshots {
    fn observe(&mut self, account: &str, ids: impl Iterator<Item = String>) -> Option<Vec<String>> {
        let current = ids.collect::<HashSet<_>>();
        let previous = self.0.insert(account.into(), current.clone())?;
        let mut ids: Vec<String> = current.difference(&previous).cloned().collect();
        ids.sort();
        (!ids.is_empty()).then_some(ids)
    }
}

#[cfg(test)]
pub(crate) fn test_service(dir: &std::path::Path) -> MailService {
    let accounts = Accounts::open(
        MailStore::open(dir).unwrap(),
        Arc::new(secrets::UnavailableSecrets),
    )
    .unwrap();
    MailService::new(accounts).unwrap()
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
        let ids = snapshots
            .observe("a", ["old".into(), "new".into()].into_iter())
            .unwrap();
        assert_eq!(ids, vec!["new".to_string()]);
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

    #[tokio::test]
    async fn busy_store_rejects_approve_send_without_holding_accounts_lock() {
        let dir = tempfile::tempdir().unwrap();
        let service = test_service(dir.path());
        let account = {
            let mut accounts = service.inner.accounts.lock().unwrap();
            accounts.protect("a strong test password").unwrap();
            accounts
                .upsert(
                    serde_json::from_value(serde_json::json!({
                        "address": "from@example.test",
                        "protocol": "jmap",
                        "session_url": "https://example.test/jmap",
                        "username": "from",
                        "secret": "secret"
                    }))
                    .unwrap(),
                )
                .unwrap()
                .id
        };
        let pending = service
            .inner
            .approvals
            .lock()
            .unwrap()
            .create(
                "key",
                &account,
                "from@example.test",
                serde_json::from_value(serde_json::json!({
                    "recipient": "a@example.test",
                    "subject": "Hi",
                    "body": "Hi"
                }))
                .unwrap(),
                api_keys::now(),
            )
            .unwrap();
        let store = service
            .inner
            .accounts
            .lock()
            .unwrap()
            .store(&account)
            .unwrap();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let busy = std::thread::spawn(move || {
            let _guard = store.lock().unwrap();
            ready_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        });
        ready_rx.recv().unwrap();
        let result = tokio::time::timeout(
            Duration::from_secs(2),
            service.approve_send(&Principal::Owner, &pending.id),
        )
        .await
        .unwrap();
        assert!(matches!(
            result,
            Err(ServiceError {
                kind: error::ErrorKind::TooManyRequests,
                ..
            })
        ));
        assert!(service.inner.accounts.try_lock().is_ok());
        assert_eq!(
            service
                .inner
                .approvals
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

    #[tokio::test]
    async fn conversation_collects_thread_across_cached_folders() {
        let dir = tempfile::tempdir().unwrap();
        let service = test_service(dir.path());
        let conv = service
            .conversation(&Principal::Owner, "demo", "weekend")
            .await
            .unwrap();
        let _ = conv;
    }
}
