//! Platform-independent email business logic and core services.
mod accounts;
mod credentials;
pub use accounts::Accounts;
pub use credentials::SecretStore;
mod jmap;
mod labels;
mod remote;
pub use jmap::JmapAccount;
pub use remote::MailAccount;

use colecto_extension_api::{Draft, DraftInput, Message};
use serde::{Deserialize, Serialize};
use std::{
    collections::HashSet,
    fs::{self, File, OpenOptions},
    io::Write,
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

enum Account {
    Imap(MailAccount),
    Jmap(JmapAccount),
}

struct PreparedConnection {
    account: Account,
    key: String,
    messages: Vec<Message>,
}

pub struct MailStore {
    directory: PathBuf,
    drafts: Vec<Draft>,
    messages: Vec<Message>,
    reader_cache: Vec<Message>,
    _lock: File,
    account: Option<Account>,
    account_key: Option<String>,
}

impl MailStore {
    pub fn open(directory: impl AsRef<Path>) -> Result<Self, Error> {
        fs::create_dir_all(directory.as_ref()).map_err(storage)?;
        let directory = fs::canonicalize(directory).map_err(storage)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join("store.lock"))
            .map_err(storage)?;
        lock.try_lock()
            .map_err(|_| Error::Storage("This data directory is already in use".into()))?;
        let drafts = match fs::read(directory.join("drafts.json")) {
            Ok(bytes) => {
                let saved: SavedMail = serde_json::from_slice(&bytes).map_err(storage)?;
                if saved.version != 1 {
                    return Err(Error::Storage("Unsupported draft store version".into()));
                }
                let mut ids = HashSet::new();
                for draft in &saved.drafts {
                    if !draft.id.starts_with("draft-")
                        || draft.id[6..].parse::<u64>().is_err()
                        || !ids.insert(&draft.id)
                    {
                        return Err(Error::Storage("Invalid or duplicate draft ID".into()));
                    }
                    validate(&draft.recipient, &draft.subject, &draft.body)?;
                }
                saved.drafts
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(error) => return Err(storage(error)),
        };
        Ok(Self {
            directory,
            drafts,
            messages: sample_messages(),
            reader_cache: Vec::new(),
            _lock: lock,
            account: None,
            account_key: None,
        })
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
    fn prepare_connection(&self, account: Account) -> Result<PreparedConnection, Error> {
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
        let path = self.directory.join("mail-cache.json");
        let mut messages = Vec::new();
        if path.exists() {
            if fs::metadata(&path).map_err(storage)?.len() > 12_000_000 {
                return Err(Error::Storage("Mail cache exceeds its size limit; preserve and remove mail-cache.json to rebuild it".into()));
            }
            let cache: MailCache =
                serde_json::from_slice(&fs::read(&path).map_err(storage)?).map_err(storage)?;
            if cache.version != 1 || cache.messages.len() > 100 {
                return Err(Error::Storage(
                    "Unsupported mail cache; preserve and remove mail-cache.json to rebuild it"
                        .into(),
                ));
            }
            if cache.account == key {
                messages = cache.messages;
            }
        }
        Ok(PreparedConnection {
            account,
            key,
            messages,
        })
    }
    fn apply_connection(&mut self, prepared: PreparedConnection) {
        self.account_key = Some(prepared.key);
        self.reader_cache.clear();
        self.messages = prepared.messages;
        self.account = Some(prepared.account);
    }
    fn cache_inbox(&self, messages: &[Message]) -> Result<(), Error> {
        let Some(account) = &self.account_key else {
            return Ok(());
        };
        let cache = MailCache {
            version: 1,
            account: account.clone(),
            messages: messages.to_vec(),
        };
        let bytes = serde_json::to_vec(&cache).map_err(storage)?;
        if bytes.len() > 12_000_000 || messages.len() > 100 {
            return Err(Error::Storage("Mail cache exceeds its size limit".into()));
        }
        let mut temporary = tempfile::NamedTempFile::new_in(&self.directory).map_err(storage)?;
        temporary.write_all(&bytes).map_err(storage)?;
        temporary.as_file().sync_all().map_err(storage)?;
        temporary
            .persist(self.directory.join("mail-cache.json"))
            .map_err(storage)?;
        Ok(())
    }
    pub fn disconnect(&mut self) {
        self.account = None;
        self.account_key = None;
        self.messages.clear();
        self.reader_cache.clear();
    }
    pub fn connected(&self) -> bool {
        self.account.is_some()
    }
    pub fn refresh(&mut self) -> Result<Vec<Message>, Error> {
        self.query("inbox", "")
    }
    pub fn query(&mut self, folder: &str, search: &str) -> Result<Vec<Message>, Error> {
        validate_query(folder, search)?;
        let search = search.trim();
        let messages = match &self.account {
            Some(Account::Imap(account)) => account.query(folder, search)?,
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
        match self
            .account
            .as_ref()
            .ok_or_else(|| Error::Invalid("Connect a mail account before sending".into()))?
        {
            Account::Imap(account) => account.send(&input),
            Account::Jmap(account) => account.send(&input),
        }
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
        validate(&input.recipient, &input.subject, &input.body)?;
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
        let draft = Draft {
            id,
            recipient: input.recipient,
            subject: input.subject,
            body: input.body,
        };
        match index {
            Some(index) => drafts[index] = draft.clone(),
            None => drafts.push(draft.clone()),
        }
        let saved = SavedMail { version: 1, drafts };
        let mut temporary = tempfile::NamedTempFile::new_in(&self.directory).map_err(storage)?;
        serde_json::to_writer_pretty(&mut temporary, &saved).map_err(storage)?;
        temporary.flush().map_err(storage)?;
        temporary.as_file().sync_all().map_err(storage)?;
        temporary
            .persist(self.directory.join("drafts.json"))
            .map_err(storage)?;
        self.drafts = saved.drafts;
        Ok(draft)
    }
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

fn validate(recipient: &str, subject: &str, body: &str) -> Result<(), Error> {
    if recipient.len() > 320 || recipient.contains(['\r', '\n']) {
        return Err(Error::Invalid(
            "Recipient must be a single line of at most 320 bytes".into(),
        ));
    }
    if subject.len() > 998 || subject.contains(['\r', '\n']) {
        return Err(Error::Invalid(
            "Subject must be a single line of at most 998 bytes".into(),
        ));
    }
    if body.len() > 100_000 {
        return Err(Error::Invalid("Body must be at most 100000 bytes".into()));
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
            !fs::read_to_string(directory.path().join("mail-cache.json"))
                .unwrap()
                .contains("secret")
        );
    }
    #[test]
    fn failed_cache_write_does_not_replace_last_successful_snapshot() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = MailStore::open(directory.path()).unwrap();
        store.connect_account(account()).unwrap();
        fs::create_dir(directory.path().join("mail-cache.json")).unwrap();
        assert!(store.cache_inbox(&sample_messages()).is_err());
        assert!(store.messages().is_empty());
    }
}
