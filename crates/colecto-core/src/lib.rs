//! Platform-independent email business logic and core services.
mod accounts;
mod credentials;
pub use accounts::Accounts;
pub use credentials::SecretStore;
mod jmap;
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

enum Account {
    Imap(MailAccount),
    Jmap(JmapAccount),
}

pub struct MailStore {
    directory: PathBuf,
    drafts: Vec<Draft>,
    messages: Vec<Message>,
    _lock: File,
    account: Option<Account>,
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
            _lock: lock,
            account: None,
        })
    }
    pub fn connect_account(&mut self, account: MailAccount) -> Result<(), Error> {
        account.validate()?;
        self.account = Some(Account::Imap(account));
        self.messages.clear();
        Ok(())
    }
    pub fn connect_jmap(&mut self, account: JmapAccount) -> Result<(), Error> {
        account.validate()?;
        self.account = Some(Account::Jmap(account));
        self.messages.clear();
        Ok(())
    }
    pub fn disconnect(&mut self) {
        self.account = None;
        self.messages.clear();
    }
    pub fn connected(&self) -> bool {
        self.account.is_some()
    }
    pub fn refresh(&mut self) -> Result<Vec<Message>, Error> {
        if let Some(account) = &self.account {
            self.messages = match account {
                Account::Imap(account) => account.receive()?,
                Account::Jmap(account) => account.receive()?,
            };
        }
        Ok(self.messages.clone())
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
        self.messages.iter().find(|m| m.id == id)
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
