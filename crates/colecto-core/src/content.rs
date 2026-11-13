//! Bounded, per-account cache of complete raw messages, so opening mail can skip the network.
//! Rows are keyed by account, provider identity and message id, and never cross providers.
use crate::{Database, Error, storage};
use std::time::{SystemTime, UNIX_EPOCH};

/// Most cached messages per account, across all of its provider identities.
pub const MAX_CONTENT_MESSAGES: usize = 1000;
/// Most cached bytes per account; the oldest stored messages are evicted first.
pub const MAX_CONTENT_BYTES: usize = 256 * 1024 * 1024;

#[derive(Clone)]
pub(crate) struct ContentCache {
    database: Database,
    account: String,
    max_messages: usize,
}

impl ContentCache {
    /// `account` was already validated by `MailCacheStore::with_database`.
    pub(crate) fn new(database: Database, account: &str) -> Self {
        Self {
            database,
            account: account.into(),
            max_messages: MAX_CONTENT_MESSAGES,
        }
    }
    pub(crate) fn get(&self, provider: &str, id: &str) -> Result<Option<Vec<u8>>, Error> {
        let connection = self.database.connection()?;
        futures_executor::block_on(async {
            let mut rows = connection
                .query(
                    "SELECT raw FROM message_content WHERE account=?1 AND provider=?2 AND id=?3",
                    [self.account.as_str(), provider, id],
                )
                .await
                .map_err(storage)?;
            let Some(row) = rows.next().await.map_err(storage)? else {
                return Ok(None);
            };
            let raw: Vec<u8> = row.get(0).map_err(storage)?;
            if raw.len() > crate::mime::MAX_RAW {
                return Err(storage("Cached message exceeds its size limit"));
            }
            Ok(Some(raw))
        })
    }
    /// The ids from `ids` that are not cached yet, in the given order.
    pub(crate) fn missing(&self, provider: &str, ids: &[String]) -> Result<Vec<String>, Error> {
        let cached = self.ids(provider)?;
        Ok(ids
            .iter()
            .filter(|id| !cached.contains(*id))
            .cloned()
            .collect())
    }
    fn ids(&self, provider: &str) -> Result<std::collections::HashSet<String>, Error> {
        let connection = self.database.connection()?;
        futures_executor::block_on(async {
            let mut rows = connection
                .query(
                    "SELECT id FROM message_content WHERE account=?1 AND provider=?2",
                    [self.account.as_str(), provider],
                )
                .await
                .map_err(storage)?;
            let mut ids = std::collections::HashSet::new();
            while let Some(row) = rows.next().await.map_err(storage)? {
                ids.insert(row.get::<String>(0).map_err(storage)?);
            }
            Ok(ids)
        })
    }
    /// Store one message and evict the oldest rows until the account is within its bounds.
    pub(crate) fn put(&self, provider: &str, id: &str, raw: &[u8]) -> Result<(), Error> {
        self.put_many(provider, &[(id.to_owned(), raw.to_vec())])
    }
    /// Store several messages in one transaction, then evict the oldest rows.
    pub(crate) fn put_many(
        &self,
        provider: &str,
        messages: &[(String, Vec<u8>)],
    ) -> Result<(), Error> {
        if messages
            .iter()
            .any(|(id, raw)| raw.len() > crate::mime::MAX_RAW || id.len() > 512)
        {
            return Err(storage("Message is too large to cache"));
        }
        if messages.is_empty() {
            return Ok(());
        }
        let stored_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let mut connection = self.database.connection()?;
        futures_executor::block_on(async {
            let transaction = connection.transaction().await.map_err(storage)?;
            for (id, raw) in messages {
                transaction
                    .execute(
                        "INSERT OR REPLACE INTO message_content(account,provider,id,stored_at,size,raw) VALUES (?1,?2,?3,?4,?5,?6)",
                        turso::params![self.account.as_str(), provider, id.as_str(), stored_at, raw.len() as i64, raw.as_slice()],
                    )
                    .await
                    .map_err(storage)?;
            }
            let mut rows = transaction
                .query(
                    "SELECT provider,id,size FROM message_content WHERE account=?1 ORDER BY stored_at DESC",
                    [self.account.as_str()],
                )
                .await
                .map_err(storage)?;
            let (mut count, mut bytes) = (0usize, 0usize);
            let mut evict = Vec::new();
            while let Some(row) = rows.next().await.map_err(storage)? {
                let size = row.get::<i64>(2).map_err(storage)?.max(0) as usize;
                count += 1;
                bytes = bytes.saturating_add(size);
                if count > self.max_messages || bytes > MAX_CONTENT_BYTES {
                    evict.push((
                        row.get::<String>(0).map_err(storage)?,
                        row.get::<String>(1).map_err(storage)?,
                    ));
                }
            }
            drop(rows);
            for (provider, id) in evict {
                transaction
                    .execute(
                        "DELETE FROM message_content WHERE account=?1 AND provider=?2 AND id=?3",
                        [self.account.as_str(), provider.as_str(), id.as_str()],
                    )
                    .await
                    .map_err(storage)?;
            }
            transaction.commit().await.map_err(storage)
        })
    }
    pub(crate) fn remove(&self, provider: &str, ids: &[String]) -> Result<(), Error> {
        let mut connection = self.database.connection()?;
        futures_executor::block_on(async {
            let transaction = connection.transaction().await.map_err(storage)?;
            for id in ids {
                transaction
                    .execute(
                        "DELETE FROM message_content WHERE account=?1 AND provider=?2 AND id=?3",
                        [self.account.as_str(), provider, id.as_str()],
                    )
                    .await
                    .map_err(storage)?;
            }
            transaction.commit().await.map_err(storage)
        })
    }
    /// Drop every row that belongs to another provider identity of this account.
    pub(crate) fn keep_provider(&self, provider: &str) -> Result<(), Error> {
        let connection = self.database.connection()?;
        futures_executor::block_on(async {
            connection
                .execute(
                    "DELETE FROM message_content WHERE account=?1 AND provider<>?2",
                    [self.account.as_str(), provider],
                )
                .await
                .map_err(storage)
                .map(|_| ())
        })
    }
    /// After an IMAP UIDVALIDITY change, drop the folder's messages from the old identity.
    pub(crate) fn keep_identity(
        &self,
        provider: &str,
        folder: &str,
        identity: &str,
    ) -> Result<(), Error> {
        let stale: Vec<String> = self
            .ids(provider)?
            .into_iter()
            .filter(|id| crate::imap_folder_id(id, folder) && !id.starts_with(identity))
            .collect();
        self.remove(provider, &stale)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn content_is_provider_scoped_bounded_and_identity_aware() {
        let directory = tempfile::tempdir().unwrap();
        let database = Database::open(directory.path()).unwrap();
        let cache = ContentCache::new(database.clone(), "a");
        let other = ContentCache::new(database, "b");
        cache.put("p", "imap-7-1", b"one").unwrap();
        cache.put("p", "imap-7-2", b"two").unwrap();
        cache.put("p", "imap-4f-7-3", b"sent").unwrap();
        assert_eq!(cache.get("p", "imap-7-1").unwrap().unwrap(), b"one");
        assert!(cache.get("q", "imap-7-1").unwrap().is_none());
        assert!(other.get("p", "imap-7-1").unwrap().is_none());
        assert_eq!(
            cache
                .missing("p", &["imap-7-2".into(), "imap-7-9".into()])
                .unwrap(),
            vec!["imap-7-9".to_owned()]
        );
        // A new Inbox UIDVALIDITY removes old Inbox rows but keeps other folders.
        cache.keep_identity("p", "inbox", "imap-8-").unwrap();
        assert!(cache.get("p", "imap-7-1").unwrap().is_none());
        assert!(cache.get("p", "imap-4f-7-3").unwrap().is_some());
        cache.put("q", "jmap-1", b"other provider").unwrap();
        cache.keep_provider("q").unwrap();
        assert!(cache.get("p", "imap-4f-7-3").unwrap().is_none());
        assert!(cache.get("q", "jmap-1").unwrap().is_some());
        assert!(
            cache
                .put("q", "huge", &vec![0; crate::mime::MAX_RAW + 1])
                .is_err()
        );
    }
    #[test]
    fn oldest_messages_are_evicted_past_the_count_bound() {
        let directory = tempfile::tempdir().unwrap();
        let mut cache = ContentCache::new(Database::open(directory.path()).unwrap(), "a");
        cache.max_messages = 3;
        for index in 0..6 {
            cache.put("p", &format!("jmap-{index}"), b"x").unwrap();
            // Distinct timestamps keep eviction order deterministic.
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        assert!(cache.get("p", "jmap-0").unwrap().is_none());
        assert!(cache.get("p", "jmap-2").unwrap().is_none());
        assert!(cache.get("p", "jmap-3").unwrap().is_some());
        assert_eq!(cache.ids("p").unwrap().len(), 3);
    }
}
