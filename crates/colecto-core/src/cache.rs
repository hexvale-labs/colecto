//! Account-local, bounded snapshots backed by the embedded Turso engine.
use crate::{Error, MailCache, Message, message_bytes, storage};
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
    connection: turso::Connection,
}
impl MailCacheStore {
    pub fn open(directory: &Path) -> Result<Self, Error> {
        // Protect the containing directory before Turso can create its database or WAL.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).map_err(storage)?;
        }
        futures_executor::block_on(async {
            let database = turso::Builder::new_local(
                directory
                    .join("mail-cache.db")
                    .to_str()
                    .ok_or_else(|| Error::Storage("Invalid cache path".into()))?,
            )
            .build()
            .await
            .map_err(storage)?;
            let connection = database.connect().map_err(storage)?;
            let mut rows = connection
                .query("PRAGMA user_version", ())
                .await
                .map_err(storage)?;
            let version = rows
                .next()
                .await
                .map_err(storage)?
                .ok_or_else(|| Error::Storage("Missing cache version".into()))?
                .get::<i64>(0)
                .map_err(storage)?;
            drop(rows);
            if !(0..=1).contains(&version) {
                return Err(Error::Storage(
                    "Unsupported mail cache database version".into(),
                ));
            }
            connection.execute_batch("CREATE TABLE IF NOT EXISTS snapshots (provider TEXT NOT NULL, folder TEXT NOT NULL, fetched_at INTEGER, messages TEXT NOT NULL, PRIMARY KEY(provider,folder)); CREATE TABLE IF NOT EXISTS migrations (name TEXT PRIMARY KEY); PRAGMA user_version=1;").await.map_err(storage)?;
            Ok(Self { connection })
        })
    }
    pub fn load(&self, provider: &str, folder: &str) -> Result<MailSnapshot, Error> {
        crate::validate_query(folder, "")?;
        futures_executor::block_on(async {
            let mut rows = self.connection.query("SELECT folder, fetched_at, messages FROM snapshots WHERE provider=?1 AND folder=?2", [provider, folder]).await.map_err(storage)?;
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
            Ok(MailSnapshot {
                folder: row.get(0).map_err(storage)?,
                fetched_at: row
                    .get::<Option<i64>>(1)
                    .map_err(storage)?
                    .map(|n| n as u64),
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
        futures_executor::block_on(async {
            let transaction = self.connection.transaction().await.map_err(storage)?;
            transaction
                .execute("DELETE FROM snapshots WHERE provider<>?1", [provider])
                .await
                .map_err(storage)?;
            let mut rows = transaction
                .query(
                    "SELECT messages FROM snapshots WHERE provider=?1 AND folder<>?2",
                    [provider, folder],
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
                        "DELETE FROM snapshots WHERE provider=?1 AND folder<>?2",
                        [provider, folder],
                    )
                    .await
                    .map_err(storage)?;
            }
            transaction.execute("INSERT OR REPLACE INTO snapshots(provider,folder,fetched_at,messages) VALUES (?1,?2,?3,?4)", turso::params![provider, folder, fetched_at as i64, text]).await.map_err(storage)?;
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
        let bytes: usize = snapshots
            .values()
            .flat_map(|s| &s.messages)
            .map(message_bytes)
            .sum();
        if count > 100 || bytes > MAX_BYTES {
            return Err(limit());
        }
        Ok(snapshots)
    }
    pub fn import_legacy(&mut self, directory: &Path) -> Result<(), Error> {
        futures_executor::block_on(async {
            let mut rows = self
                .connection
                .query("SELECT name FROM migrations WHERE name='json-v1'", ())
                .await
                .map_err(storage)?;
            if rows.next().await.map_err(storage)?.is_some() {
                return Ok(());
            }
            drop(rows);
            let path = directory.join("mail-cache.json");
            let legacy = match fs::metadata(&path) {
                Ok(metadata) => {
                    if metadata.len() > MAX_BYTES as u64 {
                        return Err(limit());
                    }
                    let cache: MailCache =
                        serde_json::from_slice(&fs::read(path).map_err(storage)?)
                            .map_err(storage)?;
                    if cache.version != 1 {
                        return Err(Error::Storage(
                            "Unsupported legacy mail cache version".into(),
                        ));
                    }
                    let text = bounded_json(&cache.messages)?;
                    Some((cache.account, text))
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(error) => return Err(storage(error)),
            };
            let transaction = self.connection.transaction().await.map_err(storage)?;
            if let Some((provider, text)) = legacy {
                transaction.execute("INSERT INTO snapshots(provider,folder,fetched_at,messages) SELECT ?1,'inbox',NULL,?2 WHERE NOT EXISTS (SELECT 1 FROM snapshots)", [provider, text]).await.map_err(storage)?;
            }
            transaction
                .execute("INSERT INTO migrations(name) VALUES ('json-v1')", ())
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
        futures_executor::block_on(cache.connection.execute("PRAGMA query_only=1", ())).unwrap();
        assert!(
            cache
                .store("after", "inbox", &crate::sample_messages())
                .is_err()
        );
        assert_eq!(cache.load("before", "inbox").unwrap().messages.len(), 4);
        futures_executor::block_on(cache.connection.execute("PRAGMA query_only=0", ())).unwrap();
        for provider in 0..5 {
            cache
                .store(&provider.to_string(), "inbox", &crate::sample_messages())
                .unwrap();
        }
        assert!(cache.load("before", "inbox").unwrap().messages.is_empty());
        let count = futures_executor::block_on(async {
            cache
                .connection
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
        futures_executor::block_on(cache.connection.execute("PRAGMA user_version=2", ())).unwrap();
        drop(cache);
        assert!(MailCacheStore::open(directory.path()).is_err());
    }
    #[test]
    fn transaction_failure_preserves_source_and_retries() {
        let directory = tempfile::tempdir().unwrap();
        let mut cache = MailCacheStore::open(directory.path()).unwrap();
        let source = br#"{"version":1,"account":"a","messages":[]}"#;
        fs::write(directory.path().join("mail-cache.json"), source).unwrap();
        futures_executor::block_on(cache.connection.execute("PRAGMA query_only=1", ())).unwrap();
        assert!(cache.import_legacy(directory.path()).is_err());
        assert_eq!(
            fs::read(directory.path().join("mail-cache.json")).unwrap(),
            source
        );
        futures_executor::block_on(cache.connection.execute("PRAGMA query_only=0", ())).unwrap();
        cache.import_legacy(directory.path()).unwrap();
        assert_eq!(cache.load("a", "inbox").unwrap().folder, "inbox");
    }
}
