//! Account-namespaced, bounded snapshots in the shared application database.
use crate::{Database, Error, MailCache, Message, message_bytes, storage};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::Path,
    sync::{Arc, RwLock},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_BYTES: usize = 12_000_000;
#[derive(Clone, Default, Serialize, Deserialize)]
pub struct MailSnapshot {
    pub folder: String,
    pub messages: Vec<Message>,
    pub fetched_at: Option<u64>,
}
pub type SnapshotHandle = Arc<RwLock<std::collections::BTreeMap<String, MailSnapshot>>>;

pub struct MailCacheStore {
    database: Database,
    account: String,
}
impl MailCacheStore {
    pub fn open(directory: &Path) -> Result<Self, Error> {
        Self::with_database(Database::open(directory)?, "demo")
    }
    pub fn with_database(database: Database, account: &str) -> Result<Self, Error> {
        if account.is_empty()
            || account.len() > 128
            || !account
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err(storage("Invalid cache account identifier"));
        }
        Ok(Self {
            database,
            account: account.into(),
        })
    }
    pub(crate) fn observe_identity(
        &mut self,
        provider: &str,
        folder: &str,
        identity: &str,
    ) -> Result<bool, Error> {
        crate::validate_query(folder, "")?;
        let mut connection = self.database.connection()?;
        futures_executor::block_on(async {
            let transaction = connection.transaction().await.map_err(storage)?;
            let mut rows = transaction.query("SELECT identity FROM mailbox_identity WHERE account=?1 AND provider=?2 AND folder=?3",[self.account.as_str(),provider,folder]).await.map_err(storage)?;
            let previous = rows
                .next()
                .await
                .map_err(storage)?
                .map(|row| row.get::<String>(0))
                .transpose()
                .map_err(storage)?;
            drop(rows);
            let changed = previous.as_deref() != Some(identity);
            if changed {
                transaction
                    .execute(
                        "DELETE FROM snapshots WHERE account=?1 AND provider=?2 AND folder=?3",
                        [self.account.as_str(), provider, folder],
                    )
                    .await
                    .map_err(storage)?;
                transaction.execute("INSERT OR REPLACE INTO mailbox_identity(account,provider,folder,identity) VALUES (?1,?2,?3,?4)",turso::params![self.account.as_str(),provider,folder,identity]).await.map_err(storage)?;
            }
            transaction.commit().await.map_err(storage)?;
            Ok(changed)
        })
    }
    pub fn load(&self, provider: &str, folder: &str) -> Result<MailSnapshot, Error> {
        crate::validate_query(folder, "")?;
        let connection = self.database.connection()?;
        futures_executor::block_on(async {
            let mut rows = connection.query("SELECT folder, fetched_at, messages FROM snapshots WHERE account=?1 AND provider=?2 AND folder=?3", [self.account.as_str(), provider, folder]).await.map_err(storage)?;
            let Some(row) = rows.next().await.map_err(storage)? else {
                return Ok(MailSnapshot::default());
            };
            let text: String = row.get(2).map_err(storage)?;
            if text.len() > MAX_BYTES {
                return Err(limit());
            }
            let messages: Vec<Message> = serde_json::from_str(&text).map_err(storage)?;
            if messages.len() > 100 {
                return Err(limit());
            }
            let fetched_at = row.get::<Option<i64>>(1).map_err(storage)?;
            if fetched_at.is_some_and(|time| time < 0) {
                return Err(storage("Invalid cache timestamp"));
            }
            Ok(MailSnapshot {
                folder: row.get(0).map_err(storage)?,
                fetched_at: fetched_at.map(|time| time as u64),
                messages,
            })
        })
    }
    pub fn store(
        &mut self,
        provider: &str,
        folder: &str,
        messages: &[Message],
    ) -> Result<(), Error> {
        crate::validate_query(folder, "")?;
        let text = bounded_json(messages)?;
        let fetched_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let mut connection = self.database.connection()?;
        futures_executor::block_on(async {
            let transaction = connection.transaction().await.map_err(storage)?;
            transaction
                .execute(
                    "DELETE FROM snapshots WHERE account=?1 AND provider<>?2",
                    [self.account.as_str(), provider],
                )
                .await
                .map_err(storage)?;
            transaction
                .execute(
                    "DELETE FROM mailbox_identity WHERE account=?1 AND provider<>?2",
                    [self.account.as_str(), provider],
                )
                .await
                .map_err(storage)?;
            let mut rows = transaction
                .query(
                    "SELECT messages FROM snapshots WHERE account=?1 AND provider=?2 AND folder<>?3",
                    [self.account.as_str(),provider, folder],
                )
                .await
                .map_err(storage)?;
            let mut count = messages.len();
            let mut bytes = text.len();
            while let Some(row) = rows.next().await.map_err(storage)? {
                let previous: String = row.get(0).map_err(storage)?;
                bytes = bytes.saturating_add(previous.len());
                count = count.saturating_add(
                    serde_json::from_str::<Vec<Message>>(&previous)
                        .map_err(storage)?
                        .len(),
                );
            }
            drop(rows);
            if count > 100 || bytes > MAX_BYTES {
                transaction
                    .execute(
                        "DELETE FROM snapshots WHERE account=?1 AND provider=?2 AND folder<>?3",
                        [self.account.as_str(), provider, folder],
                    )
                    .await
                    .map_err(storage)?;
            }
            transaction.execute("INSERT OR REPLACE INTO snapshots(account,provider,folder,fetched_at,messages) VALUES (?1,?2,?3,?4,?5)", turso::params![self.account.as_str(),provider, folder, fetched_at as i64, text]).await.map_err(storage)?;
            transaction.commit().await.map_err(storage)
        })?;
        Ok(())
    }
    pub fn snapshots(
        &self,
        provider: &str,
    ) -> Result<std::collections::BTreeMap<String, MailSnapshot>, Error> {
        let mut snapshots = std::collections::BTreeMap::new();
        for folder in ["inbox", "sent"] {
            let snapshot = self.load(provider, folder)?;
            if !snapshot.folder.is_empty() {
                snapshots.insert(folder.into(), snapshot);
            }
        }
        let count: usize = snapshots.values().map(|s| s.messages.len()).sum();
        let bytes = snapshots.values().try_fold(0usize, |sum, snapshot| {
            bounded_json(&snapshot.messages).map(|text| sum + text.len())
        })?;
        if count > 100 || bytes > MAX_BYTES {
            return Err(limit());
        }
        Ok(snapshots)
    }
    pub fn import_legacy(&mut self, directory: &Path) -> Result<(), Error> {
        let namespace = format!("account:{}", self.account);
        if self.database.contains_record(&namespace, "cache-import")? {
            if self
                .database
                .get_record(&namespace, "cache-import", 1)?
                .as_deref()
                != Some(&[1])
            {
                return Err(storage("Invalid cache migration marker"));
            }
            return Ok(());
        }
        let mut imported = Vec::new();
        let path = directory.join("mail-cache.db");
        if path.exists() {
            imported = futures_executor::block_on(async {
                let legacy = turso::Builder::new_local(
                    path.to_str()
                        .ok_or_else(|| storage("Invalid legacy cache path"))?,
                )
                .build()
                .await
                .map_err(storage)?;
                let connection = legacy.connect().map_err(storage)?;
                let mut rows = connection
                    .query("PRAGMA user_version", ())
                    .await
                    .map_err(storage)?;
                let version = rows
                    .next()
                    .await
                    .map_err(storage)?
                    .ok_or_else(|| storage("Missing cache version"))?
                    .get::<i64>(0)
                    .map_err(storage)?;
                if version != 1 {
                    return Err(storage("Unsupported mail cache database version"));
                }
                drop(rows);
                let mut rows = connection
                    .query(
                        "SELECT provider,folder,fetched_at,messages FROM snapshots",
                        (),
                    )
                    .await
                    .map_err(storage)?;
                let mut imported = Vec::new();
                while let Some(row) = rows.next().await.map_err(storage)? {
                    let provider: String = row.get(0).map_err(storage)?;
                    let folder: String = row.get(1).map_err(storage)?;
                    crate::validate_query(&folder, "")?;
                    let time: Option<i64> = row.get(2).map_err(storage)?;
                    if time.is_some_and(|t| t < 0) || provider.is_empty() || provider.len() > 8192 {
                        return Err(storage("Invalid legacy cache identity"));
                    }
                    let text: String = row.get(3).map_err(storage)?;
                    if text.len() > MAX_BYTES {
                        return Err(limit());
                    }
                    let messages: Vec<Message> = serde_json::from_str(&text).map_err(storage)?;
                    bounded_json(&messages)?;
                    imported.push((provider, folder, time, text));
                    if imported.len() > 2 {
                        return Err(limit());
                    }
                }
                Ok(imported)
            })?;
        } else {
            let path = directory.join("mail-cache.json");
            match fs::metadata(&path) {
                Ok(metadata) => {
                    if metadata.len() > MAX_BYTES as u64 {
                        return Err(limit());
                    }
                    let cache: MailCache =
                        serde_json::from_slice(&fs::read(path).map_err(storage)?)
                            .map_err(storage)?;
                    if cache.version != 1 {
                        return Err(storage("Unsupported legacy mail cache version"));
                    }
                    if cache.account.is_empty() || cache.account.len() > 8192 {
                        return Err(storage("Invalid legacy cache identity"));
                    }
                    imported.push((
                        cache.account,
                        "inbox".into(),
                        None,
                        bounded_json(&cache.messages)?,
                    ));
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
                Err(error) => return Err(storage(error)),
            }
        }
        let mut count = 0;
        let mut bytes = 0;
        for (provider, _, _, text) in &imported {
            if provider != &imported[0].0 {
                return Err(storage("Conflicting legacy cache providers"));
            }
            count += serde_json::from_str::<Vec<Message>>(text)
                .map_err(storage)?
                .len();
            bytes += text.len();
        }
        if count > 100 || bytes > MAX_BYTES {
            return Err(limit());
        }
        let mut connection = self.database.connection()?;
        futures_executor::block_on(async {
            let transaction = connection.transaction().await.map_err(storage)?;
            let mut rows = transaction
                .query(
                    "SELECT 1 FROM snapshots WHERE account=?1",
                    [self.account.as_str()],
                )
                .await
                .map_err(storage)?;
            let populated = rows.next().await.map_err(storage)?.is_some();
            drop(rows);
            if !populated {
                for (provider, folder, time, text) in imported {
                    transaction.execute("INSERT INTO snapshots(account,provider,folder,fetched_at,messages) VALUES (?1,?2,?3,?4,?5)",turso::params![self.account.as_str(),provider,folder,time,text]).await.map_err(storage)?;
                }
            }
            transaction.execute("INSERT OR IGNORE INTO records(namespace,name,version,value) VALUES (?1,'cache-import',1,?2)",turso::params![namespace.as_str(),vec![1u8]]).await.map_err(storage)?;
            transaction
                .execute(
                    "INSERT OR IGNORE INTO initialized(namespace,name) VALUES (?1,'cache-import')",
                    [namespace.as_str()],
                )
                .await
                .map_err(storage)?;
            transaction.commit().await.map_err(storage)
        })
    }
}
fn limit() -> Error {
    Error::Storage("Mail cache exceeds its size limit".into())
}
fn bounded_json(messages: &[Message]) -> Result<String, Error> {
    if messages.len() > 100
        || messages
            .iter()
            .try_fold(0usize, |sum, m| sum.checked_add(message_bytes(m)))
            .is_none_or(|size| size > MAX_BYTES)
    {
        return Err(limit());
    }
    struct Bounded(Vec<u8>);
    impl std::io::Write for Bounded {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if bytes.len() > MAX_BYTES - self.0.len() {
                return Err(std::io::Error::other("Mail cache exceeds its size limit"));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut output = Bounded(Vec::new());
    serde_json::to_writer(&mut output, messages).map_err(storage)?;
    String::from_utf8(output.0).map_err(storage)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn legacy_turso_snapshots_preserve_timestamps_and_stop_reading_sources() {
        let root = tempfile::tempdir().unwrap();
        let source = tempfile::tempdir().unwrap();
        let text = bounded_json(&crate::sample_messages()).unwrap();
        futures_executor::block_on(async {
            let legacy =
                turso::Builder::new_local(source.path().join("mail-cache.db").to_str().unwrap())
                    .build()
                    .await
                    .unwrap();
            let connection = legacy.connect().unwrap();
            connection.execute_batch("CREATE TABLE snapshots(provider TEXT NOT NULL, folder TEXT NOT NULL, fetched_at INTEGER, messages TEXT NOT NULL, PRIMARY KEY(provider,folder)); PRAGMA user_version=1;").await.unwrap();
            for folder in ["inbox", "sent"] {
                connection
                    .execute(
                        "INSERT INTO snapshots VALUES ('provider',?1,1234,?2)",
                        [folder, text.as_str()],
                    )
                    .await
                    .unwrap();
            }
        });
        let mut cache =
            MailCacheStore::with_database(Database::open(root.path()).unwrap(), "a").unwrap();
        cache.import_legacy(source.path()).unwrap();
        assert!(source.path().join("mail-cache.db").exists());
        assert_eq!(cache.snapshots("provider").unwrap().len(), 2);
        assert_eq!(
            cache.load("provider", "inbox").unwrap().fetched_at,
            Some(1234)
        );
        cache.store("provider", "inbox", &[]).unwrap();
        drop(cache);
        let mut cache =
            MailCacheStore::with_database(Database::open(root.path()).unwrap(), "a").unwrap();
        cache.import_legacy(source.path()).unwrap();
        assert!(cache.load("provider", "inbox").unwrap().messages.is_empty());
        assert_eq!(cache.load("provider", "sent").unwrap().messages.len(), 4);
    }
    #[test]
    fn missing_import_marker_cannot_restore_invalidated_legacy_snapshots() {
        let directory = tempfile::tempdir().unwrap();
        let source = serde_json::to_vec(&MailCache {
            version: 1,
            account: "p".into(),
            messages: crate::sample_messages(),
        })
        .unwrap();
        fs::write(directory.path().join("mail-cache.json"), &source).unwrap();
        let mut cache = MailCacheStore::open(directory.path()).unwrap();
        cache.import_legacy(directory.path()).unwrap();
        assert_eq!(cache.load("p", "inbox").unwrap().messages.len(), 4);
        cache.observe_identity("p", "inbox", "imap-7-").unwrap();
        assert!(cache.observe_identity("p", "inbox", "imap-8-").unwrap());
        {
            let connection = cache.database.connection().unwrap();
            futures_executor::block_on(connection.execute(
                "DELETE FROM records WHERE namespace='account:demo' AND name='cache-import'",
                (),
            ))
            .unwrap();
        }
        drop(cache);
        let mut cache = MailCacheStore::open(directory.path()).unwrap();
        assert!(cache.import_legacy(directory.path()).is_err());
        assert!(cache.load("p", "inbox").unwrap().messages.is_empty());
        assert_eq!(
            fs::read(directory.path().join("mail-cache.json")).unwrap(),
            source
        );
    }
    #[test]
    fn shared_cache_isolates_accounts_and_invalidates_only_changed_mailbox() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path()).unwrap();
        let mut a = MailCacheStore::with_database(database.clone(), "a").unwrap();
        let mut b = MailCacheStore::with_database(database, "b").unwrap();
        a.observe_identity("p", "inbox", "imap-7-").unwrap();
        a.store("p", "inbox", &crate::sample_messages()).unwrap();
        a.store("p", "sent", &crate::sample_messages()).unwrap();
        b.store("p", "inbox", &crate::sample_messages()).unwrap();
        assert!(!a.observe_identity("p", "inbox", "imap-7-").unwrap());
        assert!(a.observe_identity("p", "inbox", "imap-8-").unwrap());
        assert!(a.load("p", "inbox").unwrap().messages.is_empty());
        assert_eq!(a.load("p", "sent").unwrap().messages.len(), 4);
        assert_eq!(b.load("p", "inbox").unwrap().messages.len(), 4);
    }
    #[test]
    fn bounds_preserve_previous_transaction_and_identity() {
        let directory = tempfile::tempdir().unwrap();
        let mut cache = MailCacheStore::open(directory.path()).unwrap();
        cache
            .store("provider-a", "inbox", &crate::sample_messages())
            .unwrap();
        assert!(
            cache
                .load("provider-b", "inbox")
                .unwrap()
                .messages
                .is_empty()
        );
        assert!(
            cache
                .store(
                    "provider-a",
                    "sent",
                    &vec![crate::sample_messages()[0].clone(); 101]
                )
                .is_err()
        );
        let mut huge = crate::sample_messages();
        huge[0].body = "x".repeat(MAX_BYTES + 1);
        assert!(cache.store("provider-a", "sent", &huge).is_err());
        huge[0].body = "\"".repeat(MAX_BYTES / 2);
        assert!(cache.store("provider-a", "sent", &huge).is_err());
        let snapshot = cache.load("provider-a", "inbox").unwrap();
        assert_eq!(snapshot.messages.len(), 4);
        assert_eq!(snapshot.folder, "inbox");
        assert!(snapshot.fetched_at.is_some());
    }
    #[test]
    fn provider_switches_bound_history_and_failed_writes_preserve_previous_provider() {
        let directory = tempfile::tempdir().unwrap();
        let mut cache = MailCacheStore::open(directory.path()).unwrap();
        cache
            .store("before", "inbox", &crate::sample_messages())
            .unwrap();
        futures_executor::block_on(
            cache
                .database
                .connection()
                .unwrap()
                .execute("PRAGMA query_only=1", ()),
        )
        .unwrap();
        assert!(
            cache
                .store("after", "inbox", &crate::sample_messages())
                .is_err()
        );
        assert_eq!(cache.load("before", "inbox").unwrap().messages.len(), 4);
        futures_executor::block_on(
            cache
                .database
                .connection()
                .unwrap()
                .execute("PRAGMA query_only=0", ()),
        )
        .unwrap();
        for provider in 0..5 {
            cache
                .store(&provider.to_string(), "inbox", &crate::sample_messages())
                .unwrap();
        }
        assert!(cache.load("before", "inbox").unwrap().messages.is_empty());
        let connection = cache.database.connection().unwrap();
        let count = futures_executor::block_on(async {
            connection
                .query("SELECT COUNT(*) FROM snapshots", ())
                .await
                .unwrap()
                .next()
                .await
                .unwrap()
                .unwrap()
                .get::<i64>(0)
                .unwrap()
        });
        assert_eq!(count, 1);
    }
    #[test]
    fn folders_are_retained_until_aggregate_pressure_requires_eviction() {
        let directory = tempfile::tempdir().unwrap();
        let mut cache = MailCacheStore::open(directory.path()).unwrap();
        cache
            .store("a", "inbox", &crate::sample_messages())
            .unwrap();
        cache.store("a", "sent", &crate::sample_messages()).unwrap();
        assert_eq!(cache.snapshots("a").unwrap().len(), 2);
        cache
            .store("a", "sent", &vec![crate::sample_messages()[0].clone(); 100])
            .unwrap();
        assert!(cache.load("a", "inbox").unwrap().messages.is_empty());
        assert_eq!(cache.load("a", "sent").unwrap().messages.len(), 100);
    }
    #[test]
    fn future_database_and_invalid_import_are_preserved() {
        let directory = tempfile::tempdir().unwrap();
        let mut cache = MailCacheStore::open(directory.path()).unwrap();
        let source = br#"{"version":2,"account":"a","messages":[]}"#;
        fs::write(directory.path().join("mail-cache.json"), source).unwrap();
        assert!(cache.import_legacy(directory.path()).is_err());
        assert_eq!(
            fs::read(directory.path().join("mail-cache.json")).unwrap(),
            source
        );
        futures_executor::block_on(
            cache
                .database
                .connection()
                .unwrap()
                .execute("PRAGMA user_version=2", ()),
        )
        .unwrap();
        drop(cache);
        assert!(MailCacheStore::open(directory.path()).is_err());
    }
    #[test]
    fn transaction_failure_preserves_source_and_retries() {
        let directory = tempfile::tempdir().unwrap();
        let mut cache = MailCacheStore::open(directory.path()).unwrap();
        let source = br#"{"version":1,"account":"a","messages":[]}"#;
        fs::write(directory.path().join("mail-cache.json"), source).unwrap();
        futures_executor::block_on(
            cache
                .database
                .connection()
                .unwrap()
                .execute("PRAGMA query_only=1", ()),
        )
        .unwrap();
        assert!(cache.import_legacy(directory.path()).is_err());
        assert_eq!(
            fs::read(directory.path().join("mail-cache.json")).unwrap(),
            source
        );
        futures_executor::block_on(
            cache
                .database
                .connection()
                .unwrap()
                .execute("PRAGMA query_only=0", ()),
        )
        .unwrap();
        cache.import_legacy(directory.path()).unwrap();
        assert_eq!(cache.load("a", "inbox").unwrap().folder, "inbox");
    }
}
