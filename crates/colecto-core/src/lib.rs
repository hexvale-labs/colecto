//! Platform-independent email business logic and core services.
mod accounts;
mod cache;
mod database;
pub use cache::{MailCacheStore, MailSnapshot, SnapshotHandle};
pub use database::Database;
mod credentials;
pub use accounts::Accounts;
pub use credentials::SecretStore;
mod jmap;
mod labels;
pub mod mime;
mod remote;
pub use jmap::JmapAccount;
pub use remote::MailAccount;

use colecto_extension_api::{AttachmentInfo, Draft, DraftInput, Message, MessageContent};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    path::{Path, PathBuf},
};

#[derive(Debug)]
pub enum Error {
    Storage(String),
    Transport(String),
    Invalid(String),
    Missing,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(message) | Self::Storage(message) | Self::Invalid(message) => {
                f.write_str(message)
            }
            Self::Missing => f.write_str("Mail item not found"),
        }
    }
}
impl std::error::Error for Error {}
fn storage(error: impl std::fmt::Display) -> Error {
    Error::Storage(error.to_string())
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedMail {
    version: u32,
    drafts: Vec<Draft>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct MailCache {
    version: u32,
    account: String,
    messages: Vec<Message>,
}

#[derive(Clone)]
enum Account {
    Imap(MailAccount),
    Jmap(JmapAccount),
}

struct PreparedConnection {
    snapshots: std::collections::BTreeMap<String, MailSnapshot>,
    account: Account,
    key: String,
    messages: Vec<Message>,
}

pub struct MailStore {
    directory: PathBuf,
    database: Database,
    namespace: String,
    drafts: Vec<Draft>,
    messages: Vec<Message>,
    reader_cache: Vec<Message>,
    _lock: File,
    cache: MailCacheStore,
    snapshot: SnapshotHandle,
    account: Option<Account>,
    account_key: Option<String>,
}

impl MailStore {
    pub fn open(directory: impl AsRef<Path>) -> Result<Self, Error> {
        Self::open_inner(directory.as_ref(), None, "demo")
    }
    pub fn open_with_database(
        directory: impl AsRef<Path>,
        database: Database,
        account: &str,
    ) -> Result<Self, Error> {
        Self::open_inner(directory.as_ref(), Some(database), account)
    }
    fn open_inner(
        directory: &Path,
        database: Option<Database>,
        account: &str,
    ) -> Result<Self, Error> {
        database::private_directory(directory)?;
        let directory = fs::canonicalize(directory).map_err(storage)?;
        let namespace = format!("account:{account}");
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("store.lock"))
            .map_err(storage)?;
        lock.try_lock()
            .map_err(|_| Error::Storage("This data directory is already in use".into()))?;
        let database = match database {
            Some(database) => database,
            None => Database::open(&directory)?,
        };
        let bytes = if database.contains_record(&namespace, "drafts")? {
            database
                .get_record(&namespace, "drafts", 1)?
                .ok_or_else(|| storage("Authoritative drafts were removed"))?
        } else {
            let path = directory.join("drafts.json");
            if fs::metadata(&path).is_ok_and(|metadata| metadata.len() > 32_000_000) {
                return Err(storage("Draft store exceeds its size limit"));
            }
            let bytes = match fs::read(path) {
                Ok(bytes) => bytes,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    serde_json::to_vec(&SavedMail {
                        version: 1,
                        drafts: vec![],
                    })
                    .map_err(storage)?
                }
                Err(error) => return Err(storage(error)),
            };
            validate_drafts(&bytes)?;
            database.initialize_record(&namespace, "drafts", 1, &bytes)?
        };
        let drafts = validate_drafts(&bytes)?;
        let mut cache = MailCacheStore::with_database(database.clone(), account)?;
        cache.import_legacy(&directory)?;
        let store = Self {
            cache,
            snapshot: std::sync::Arc::new(std::sync::RwLock::new(
                std::collections::BTreeMap::from([(
                    "inbox".into(),
                    MailSnapshot {
                        folder: "inbox".into(),
                        messages: sample_messages(),
                        fetched_at: None,
                    },
                )]),
            )),
            directory,
            database,
            namespace,
            drafts,
            messages: sample_messages(),
            reader_cache: Vec::new(),
            _lock: lock,
            account: None,
            account_key: None,
        };
        store.labels()?;
        Ok(store)
    }
    pub fn connect_account(&mut self, account: MailAccount) -> Result<(), Error> {
        let prepared = self.prepare_connection(Account::Imap(account))?;
        self.apply_connection(prepared);
        Ok(())
    }
    pub fn connect_jmap(&mut self, account: JmapAccount) -> Result<(), Error> {
        let prepared = self.prepare_connection(Account::Jmap(account))?;
        self.apply_connection(prepared);
        Ok(())
    }
    fn prepare_connection(&mut self, account: Account) -> Result<PreparedConnection, Error> {
        let key = match &account {
            Account::Imap(account) => {
                account.validate()?;
                serde_json::to_string(&(
                    "imap",
                    &account.imap_host,
                    account.imap_port,
                    &account.username,
                    &account.address,
                ))
            }
            Account::Jmap(account) => {
                account.validate()?;
                serde_json::to_string(&("jmap", &account.session_url, &account.address))
            }
        }
        .map_err(storage)?;
        self.cache.import_legacy(&self.directory)?;
        let snapshots = self.cache.snapshots(&key)?;
        let messages = snapshots
            .get("inbox")
            .map(|s| s.messages.clone())
            .unwrap_or_default();
        Ok(PreparedConnection {
            snapshots,
            account,
            key,
            messages,
        })
    }
    fn apply_connection(&mut self, prepared: PreparedConnection) {
        *self.snapshot.write().unwrap() = prepared.snapshots;
        self.account_key = Some(prepared.key);
        self.reader_cache.clear();
        self.messages = prepared.messages;
        self.account = Some(prepared.account);
    }
    fn cache_inbox(&mut self, messages: &[Message]) -> Result<(), Error> {
        let Some(account) = &self.account_key else {
            return Ok(());
        };
        self.cache.store(account, "inbox", messages)?;
        let snapshots = self.cache.snapshots(account)?;
        *self.snapshot.write().map_err(storage)? = snapshots;
        Ok(())
    }
    pub fn disconnect(&mut self) {
        self.snapshot.write().unwrap().clear();
        self.account = None;
        self.account_key = None;
        self.messages.clear();
        self.reader_cache.clear();
    }
    pub fn snapshot_handle(&self) -> SnapshotHandle {
        self.snapshot.clone()
    }
    pub fn connected(&self) -> bool {
        self.account.is_some()
    }
    /// A detached handle for push notifications, so waiting never holds this store's lock.
    pub fn push_watcher(&self) -> Option<PushWatcher> {
        Some(PushWatcher {
            account: self.account.clone()?,
            key: self.account_key.clone()?,
        })
    }
    pub fn refresh(&mut self) -> Result<Vec<Message>, Error> {
        self.query("inbox", "")
    }
    fn observe_mailbox_identity(&mut self, folder: &str, identity: &str) -> Result<(), Error> {
        if let Some(provider) = &self.account_key
            && self.cache.observe_identity(provider, folder, identity)?
        {
            self.reader_cache.retain(|message| {
                !imap_folder_id(&message.id, folder) || message.id.starts_with(identity)
            });
            if folder == "inbox" {
                self.messages
                    .retain(|message| message.id.starts_with(identity));
            }
            *self.snapshot.write().map_err(storage)? = self.cache.snapshots(provider)?;
        }
        Ok(())
    }
    pub fn query(&mut self, folder: &str, search: &str) -> Result<Vec<Message>, Error> {
        validate_query(folder, search)?;
        let search = search.trim();
        let messages = match &self.account {
            Some(Account::Imap(account)) => {
                let (messages, identity) = account.query_with_identity(folder, search)?;
                self.observe_mailbox_identity(folder, &identity)?;
                messages
            }
            Some(Account::Jmap(account)) => account.query(folder, search)?,
            None if folder == "sent" => Vec::new(),
            None => {
                let query = search.to_lowercase();
                self.messages
                    .iter()
                    .filter(|m| {
                        [&m.sender, &m.address, &m.subject, &m.body]
                            .iter()
                            .any(|text| text.to_lowercase().contains(&query))
                    })
                    .cloned()
                    .collect()
            }
        };
        if folder == "inbox" && search.is_empty() {
            self.cache_inbox(&messages)?;
        }
        if folder == "sent"
            && search.is_empty()
            && let Some(account) = &self.account_key
        {
            self.cache.store(account, folder, &messages)?;
            let snapshots = self.cache.snapshots(account)?;
            *self.snapshot.write().map_err(storage)? = snapshots;
        }
        for message in &messages {
            self.reader_cache.retain(|cached| cached.id != message.id);
            self.reader_cache.push(message.clone());
        }
        let mut bytes: usize = self.reader_cache.iter().map(message_bytes).sum();
        let mut remove = self.reader_cache.len().saturating_sub(500);
        for message in &self.reader_cache[..remove] {
            bytes -= message_bytes(message);
        }
        while bytes > 12_000_000 && remove < self.reader_cache.len() {
            bytes -= message_bytes(&self.reader_cache[remove]);
            remove += 1;
        }
        self.reader_cache.drain(..remove);
        if folder == "inbox" && search.is_empty() {
            self.messages = messages.clone();
        }
        Ok(messages)
    }
    pub fn send(&self, input: DraftInput) -> Result<(), Error> {
        let account = self
            .account
            .as_ref()
            .ok_or_else(|| Error::Invalid("Connect a mail account before sending".into()))?;
        validate_draft(&input)?;
        let mut files = mime::uploads(&input.attachments)?;
        if let Some(original) = &input.forward_of {
            files.extend(mime::attachments(&self.raw(original)?)?);
        }
        if files.iter().map(|file| file.data.len()).sum::<usize>() > mime::MAX_ATTACHMENTS {
            return Err(Error::Invalid(
                "Attachments must total at most 25 MB".into(),
            ));
        }
        match account {
            Account::Imap(account) => account.send(&input, &files),
            Account::Jmap(account) => account.send(&input, &files),
        }
    }
    fn raw(&self, id: &str) -> Result<Vec<u8>, Error> {
        match &self.account {
            Some(Account::Imap(account)) => account.fetch_raw(id),
            Some(Account::Jmap(account)) => account.fetch_raw(id),
            None => Err(Error::Missing),
        }
    }
    /// Provider-fetched HTML, attachments, and reply headers for one message.
    pub fn content(&self, id: &str) -> Result<MessageContent, Error> {
        if self.account.is_none() {
            let message = self.message(id).ok_or(Error::Missing)?;
            return Ok(MessageContent {
                text: message.body.clone(),
                ..MessageContent::default()
            });
        }
        mime::content(&self.raw(id)?)
    }
    pub fn attachment(&self, id: &str, index: usize) -> Result<(AttachmentInfo, Vec<u8>), Error> {
        let file = mime::attachment(&self.raw(id)?, index)?;
        Ok((
            AttachmentInfo {
                index,
                name: file.name,
                content_type: file.content_type,
                size: file.data.len(),
            },
            file.data,
        ))
    }
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }
    pub fn message(&self, id: &str) -> Option<&Message> {
        self.reader_cache
            .iter()
            .chain(&self.messages)
            .find(|m| m.id == id)
    }
    pub fn drafts(&self) -> &[Draft] {
        &self.drafts
    }
    pub fn draft(&self, id: &str) -> Option<&Draft> {
        self.drafts.iter().find(|d| d.id == id)
    }

    pub fn save_draft(&mut self, input: DraftInput) -> Result<Draft, Error> {
        let draft = Draft {
            id: String::new(),
            recipient: input.recipient,
            cc: input.cc,
            subject: input.subject,
            body: input.body,
            in_reply_to: input.in_reply_to,
            references: input.references,
            forward_of: input.forward_of,
        };
        validate(&draft)?;
        let mut drafts = self.drafts.clone();
        let index = match input.id.as_ref() {
            Some(id) => Some(
                drafts
                    .iter()
                    .position(|d| &d.id == id)
                    .ok_or(Error::Missing)?,
            ),
            None => None,
        };
        let id = match input.id {
            Some(id) => id,
            None => {
                let next = drafts
                    .iter()
                    .filter_map(|d| d.id.strip_prefix("draft-")?.parse::<u64>().ok())
                    .max()
                    .unwrap_or(0)
                    .checked_add(1)
                    .ok_or_else(|| Error::Invalid("Draft ID limit reached".into()))?;
                format!("draft-{next}")
            }
        };
        let draft = Draft { id, ..draft };
        match index {
            Some(index) => drafts[index] = draft.clone(),
            None => drafts.push(draft.clone()),
        }
        let saved = SavedMail { version: 1, drafts };
        self.database.set_record(
            &self.namespace,
            "drafts",
            1,
            &serde_json::to_vec(&saved).map_err(storage)?,
        )?;
        self.drafts = saved.drafts;
        Ok(draft)
    }
}

fn imap_folder_id(id: &str, folder: &str) -> bool {
    id.strip_prefix("imap-")
        .is_some_and(|rest| rest.matches('-').count() == if folder == "inbox" { 1 } else { 2 })
}

fn validate_drafts(bytes: &[u8]) -> Result<Vec<Draft>, Error> {
    if bytes.len() > 32_000_000 {
        return Err(storage("Draft store exceeds its size limit"));
    }
    let saved: SavedMail = serde_json::from_slice(bytes).map_err(storage)?;
    if saved.version != 1 {
        return Err(storage("Unsupported draft store version"));
    }
    let mut ids = HashSet::new();
    for draft in &saved.drafts {
        if !draft.id.starts_with("draft-")
            || draft.id[6..].parse::<u64>().is_err()
            || !ids.insert(&draft.id)
        {
            return Err(storage("Invalid or duplicate draft ID"));
        }
        validate(draft)?;
    }
    Ok(saved.drafts)
}

fn message_bytes(message: &Message) -> usize {
    message.id.len()
        + message.sender.len()
        + message.address.len()
        + message.subject.len()
        + message.body.len()
        + message.date.len()
}
fn bounded_body(mut body: String) -> String {
    if body.len() > 100_000 {
        let mut end = 100_000;
        while !body.is_char_boundary(end) {
            end -= 1;
        }
        body.truncate(end);
        body.push_str("\n[Body truncated]");
    }
    body
}

fn validate_query(folder: &str, search: &str) -> Result<(), Error> {
    if !matches!(folder, "inbox" | "sent")
        || search.len() > 1024
        || search.chars().any(char::is_control)
    {
        return Err(Error::Invalid(
            "Choose Inbox or Sent and a search of at most 1024 bytes without control characters"
                .into(),
        ));
    }
    Ok(())
}

/// Parse a comma-separated address list; an empty list is allowed.
/// Waits for the provider to report Inbox changes: IMAP IDLE or JMAP push.
pub struct PushWatcher {
    account: Account,
    key: String,
}
impl PushWatcher {
    /// Identifies the provider connection, so callers can tell when an account was reconfigured.
    pub fn key(&self) -> &str {
        &self.key
    }
    /// `Ok(true)` when new mail may have arrived, `Ok(false)` after a quiet `timeout`.
    /// `Error::Invalid` means the provider does not support push; keep polling instead.
    pub fn wait(&self, timeout: std::time::Duration) -> Result<bool, Error> {
        match &self.account {
            Account::Imap(account) => account.wait_for_inbox(timeout),
            Account::Jmap(account) => account.wait_for_change(timeout),
        }
    }
}

pub(crate) fn mailboxes(list: &str) -> Result<Vec<lettre::message::Mailbox>, Error> {
    if list.trim().is_empty() {
        return Ok(Vec::new());
    }
    list.parse::<lettre::message::Mailboxes>()
        .map(|mailboxes| mailboxes.into_iter().collect())
        .map_err(|_| Error::Invalid("Enter valid email addresses, separated by commas".into()))
}

/// Validate a full outgoing snapshot without persisting a draft or contacting a provider.
pub fn validate_draft(input: &DraftInput) -> Result<(), Error> {
    validate(&Draft {
        id: String::new(),
        recipient: input.recipient.clone(),
        cc: input.cc.clone(),
        subject: input.subject.clone(),
        body: input.body.clone(),
        in_reply_to: input.in_reply_to.clone(),
        references: input.references.clone(),
        forward_of: input.forward_of.clone(),
    })?;
    if mailboxes(&input.recipient)?.is_empty() {
        return Err(Error::Invalid("Enter at least one recipient".into()));
    }
    mailboxes(&input.cc)?;
    if input.id.as_ref().is_some_and(|id| id.len() > 256) {
        return Err(Error::Invalid("Invalid draft identifier".into()));
    }
    if input.attachments.len() > 20 {
        return Err(Error::Invalid("Attach at most 20 files".into()));
    }
    Ok(())
}

fn validate(draft: &Draft) -> Result<(), Error> {
    for (name, value) in [("Recipients", &draft.recipient), ("Cc", &draft.cc)] {
        if value.len() > 4000 || value.contains(['\r', '\n']) {
            return Err(Error::Invalid(format!(
                "{name} must be a single line of at most 4000 bytes"
            )));
        }
    }
    if draft.subject.len() > 998 || draft.subject.contains(['\r', '\n']) {
        return Err(Error::Invalid(
            "Subject must be a single line of at most 998 bytes".into(),
        ));
    }
    if draft.body.len() > 100_000 {
        return Err(Error::Invalid("Body must be at most 100000 bytes".into()));
    }
    if [&draft.in_reply_to, &draft.references]
        .iter()
        .any(|value| value.len() > 4000 || value.chars().any(char::is_control))
        || draft.forward_of.as_ref().is_some_and(|id| id.len() > 256)
    {
        return Err(Error::Invalid("Invalid reply or forward reference".into()));
    }
    Ok(())
}

fn sample_messages() -> Vec<Message> {
    [
        ("welcome", "The Colecto team", "hello@colecto.example", "A quieter place for your mail", "Welcome to Colecto.\n\nA little space to read, think, and write. This inbox contains sample mail, so you can explore without connecting an account.\n\nOpen a message, start a draft, and make yourself at home. Saved drafts stay on this computer, even after you restart the demo.\n\nThe Colecto team", "Nov 7"),
        ("weekend", "Maya Chen", "maya@example.test", "A small plan for the weekend", "Hey,\n\nI found a little bookshop by the river. Coffee, a walk, and no particular agenda?\n\nLet me know what you think.\n\nMaya", "Nov 6"),
        ("notes", "Studio North", "studio@example.test", "Notes from our last conversation", "A few things to keep close:\n\n• Make room for the work that matters.\n• Keep the defaults simple.\n• Leave things better than you found them.\n\nSee you next week.", "Nov 5"),
        ("plaintext", "Alex Rivera", "alex@example.test", "Plain text, with a little character", "Hello 世界 👋\n\nMail should be readable, whatever language you write in.\n\nThis is text, not a button: <button>Do not click</button>\nAnd this is just text too: <script>alert('hello')</script>\n\nAlex", "Nov 4"),
    ].into_iter().map(|(id,sender,address,subject,body,date)| Message { id:id.into(), sender:sender.into(), address:address.into(), subject:subject.into(), body:body.into(), date:date.into() }).collect()
}

#[cfg(test)]
mod cache_tests {
    use super::*;
    fn account() -> MailAccount {
        MailAccount {
            imap_host: "imap.example.test".into(),
            imap_port: 993,
            smtp_host: "smtp.example.test".into(),
            smtp_port: 587,
            username: "me".into(),
            password: "secret".into(),
            address: "me@example.test".into(),
        }
    }
    #[test]
    fn mailbox_identity_change_removes_old_reader_and_snapshot_even_for_empty_results() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = MailStore::open(dir.path()).unwrap();
        store.connect_account(account()).unwrap();
        store.observe_mailbox_identity("inbox", "imap-7-").unwrap();
        let mut old = sample_messages().remove(0);
        old.id = "imap-7-42".into();
        store.cache_inbox(&[old.clone()]).unwrap();
        store.reader_cache.push(old);
        let mut sent = sample_messages().remove(0);
        sent.id = "imap-53656e74-7-42".into();
        store.reader_cache.push(sent.clone());
        store.observe_mailbox_identity("inbox", "imap-8-").unwrap();
        assert!(store.message("imap-7-42").is_none());
        assert!(store.message(&sent.id).is_some());
        assert!(
            !store
                .snapshot_handle()
                .read()
                .unwrap()
                .contains_key("inbox")
        );
    }
    #[test]
    fn failed_draft_transaction_preserves_memory_and_canonical_state() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = MailStore::open(dir.path()).unwrap();
        let input = DraftInput {
            id: None,
            recipient: String::new(),
            subject: "Saved".into(),
            body: String::new(),
            ..Default::default()
        };
        let draft = store.save_draft(input.clone()).unwrap();
        let before = store
            .database
            .get_record(&store.namespace, "drafts", 1)
            .unwrap();
        futures_executor::block_on(
            store
                .database
                .connection()
                .unwrap()
                .execute("PRAGMA query_only=1", ()),
        )
        .unwrap();
        assert!(
            store
                .save_draft(DraftInput {
                    id: Some(draft.id.clone()),
                    subject: "Unsaved".into(),
                    ..input
                })
                .is_err()
        );
        assert_eq!(store.draft(&draft.id).unwrap().subject, "Saved");
        assert_eq!(
            store
                .database
                .get_record(&store.namespace, "drafts", 1)
                .unwrap(),
            before
        );
    }
    #[test]
    fn legacy_import_is_one_time_and_source_is_preserved() {
        let directory = tempfile::tempdir().unwrap();
        let provider = account();
        let key = serde_json::to_string(&(
            "imap",
            &provider.imap_host,
            provider.imap_port,
            &provider.username,
            &provider.address,
        ))
        .unwrap();
        let source = serde_json::to_vec(&MailCache {
            version: 1,
            account: key,
            messages: sample_messages(),
        })
        .unwrap();
        fs::write(directory.path().join("mail-cache.json"), &source).unwrap();
        let mut store = MailStore::open(directory.path()).unwrap();
        store.connect_account(account()).unwrap();
        assert_eq!(store.messages().len(), 4);
        assert!(directory.path().join("colecto.db").exists());
        assert_eq!(
            fs::read(directory.path().join("mail-cache.json")).unwrap(),
            source
        );
        store.cache_inbox(&[]).unwrap();
        drop(store);
        let mut store = MailStore::open(directory.path()).unwrap();
        store.connect_account(account()).unwrap();
        assert!(store.messages().is_empty());
    }
    #[test]
    fn reader_cache_bounds_memory_across_many_searches() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = MailStore::open(directory.path()).unwrap();
        for batch in 0..3 {
            store.messages = (0..100)
                .map(|index| {
                    let mut message = sample_messages().remove(0);
                    message.id = format!("{batch}-{index}");
                    message.body = "x".repeat(100_000);
                    message
                })
                .collect();
            store.query("inbox", "x").unwrap();
        }
        assert!(store.reader_cache.len() < 130);
        assert!(store.message("0-0").is_none());
        assert!(store.message("2-99").is_some());
    }
    #[test]
    fn explicit_disconnect_clears_independent_cached_access() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = MailStore::open(directory.path()).unwrap();
        store.connect_account(account()).unwrap();
        store.cache_inbox(&sample_messages()).unwrap();
        let snapshot = store.snapshot_handle();
        assert_eq!(snapshot.read().unwrap()["inbox"].messages.len(), 4);
        store.disconnect();
        assert!(snapshot.read().unwrap().is_empty());
        assert!(store.messages().is_empty());
        store.connect_account(account()).unwrap();
        assert_eq!(snapshot.read().unwrap()["inbox"].messages.len(), 4);
    }
    #[test]
    fn failed_refresh_preserves_last_successful_snapshot() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = MailStore::open(directory.path()).unwrap();
        let mut provider = account();
        provider.imap_host = "127.0.0.1".into();
        provider.imap_port = 1;
        store.connect_account(provider).unwrap();
        store.cache_inbox(&sample_messages()).unwrap();
        let before = store.snapshot_handle().read().unwrap()["inbox"].fetched_at;
        assert!(store.refresh().is_err());
        let snapshot = store.snapshot_handle();
        let snapshot = snapshot.read().unwrap();
        assert_eq!(snapshot["inbox"].messages.len(), 4);
        assert_eq!(snapshot["inbox"].fetched_at, before);
    }
    #[test]
    fn inbox_cache_survives_restart_but_never_crosses_provider_identity() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = MailStore::open(directory.path()).unwrap();
        store.connect_account(account()).unwrap();
        store.cache_inbox(&sample_messages()).unwrap();
        drop(store);
        let mut store = MailStore::open(directory.path()).unwrap();
        store.connect_account(account()).unwrap();
        assert_eq!(store.messages().len(), 4);
        let mut another = account();
        another.username = "another".into();
        store.connect_account(another).unwrap();
        assert!(store.messages().is_empty());
        assert!(
            !String::from_utf8_lossy(&fs::read(directory.path().join("colecto.db")).unwrap())
                .contains("secret")
        );
    }
    #[test]
    fn failed_cache_write_does_not_replace_last_successful_snapshot() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = MailStore::open(directory.path()).unwrap();
        store.connect_account(account()).unwrap();
        let mut oversized = sample_messages();
        oversized[0].body = "x".repeat(12_000_001);
        assert!(store.cache_inbox(&oversized).is_err());
        assert!(store.messages().is_empty());
    }
}
