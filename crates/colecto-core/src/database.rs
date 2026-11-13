//! Private, shared application storage. Callers validate their own serialized formats.
use crate::{Error, storage};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
};

const MAX_RECORD_BYTES: usize = 32_000_000;
const CONTENT_TABLE: &str = "CREATE TABLE IF NOT EXISTS message_content(account TEXT NOT NULL, provider TEXT NOT NULL, id TEXT NOT NULL, stored_at INTEGER NOT NULL, size INTEGER NOT NULL, raw BLOB NOT NULL, PRIMARY KEY(account,provider,id));";

#[derive(Clone)]
pub struct Database {
    path: Arc<PathBuf>,
    connection: Arc<Mutex<turso::Connection>>,
}
impl Database {
    pub fn open(directory: impl AsRef<Path>) -> Result<Self, Error> {
        private_directory(directory.as_ref())?;
        let path = fs::canonicalize(directory)
            .map_err(storage)?
            .join("colecto.db");
        if fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err(Error::Storage(
                "Database must not be a symbolic link".into(),
            ));
        }
        let connection = futures_executor::block_on(async {
            let database = turso::Builder::new_local(
                path.to_str()
                    .ok_or_else(|| storage("Invalid database path"))?,
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
                .ok_or_else(|| storage("Missing database version"))?
                .get::<i64>(0)
                .map_err(storage)?;
            drop(rows);
            if !(0..=1).contains(&version) {
                return Err(storage("Unsupported application database version"));
            }
            if version == 0 {
                connection.execute_batch("CREATE TABLE IF NOT EXISTS initialized(namespace TEXT NOT NULL, name TEXT NOT NULL, PRIMARY KEY(namespace,name)); CREATE TABLE IF NOT EXISTS records(namespace TEXT NOT NULL, name TEXT NOT NULL, version INTEGER NOT NULL, value BLOB, PRIMARY KEY(namespace,name)); CREATE TABLE IF NOT EXISTS mailbox_identity(account TEXT NOT NULL, provider TEXT NOT NULL, folder TEXT NOT NULL, identity TEXT NOT NULL, PRIMARY KEY(account,provider,folder)); CREATE TABLE IF NOT EXISTS snapshots(account TEXT NOT NULL, provider TEXT NOT NULL, folder TEXT NOT NULL, fetched_at INTEGER, messages TEXT NOT NULL, PRIMARY KEY(account,provider,folder)); PRAGMA user_version=1;").await.map_err(storage)?;
            } else {
                for sql in [
                    "SELECT namespace,name,version,value FROM records LIMIT 0",
                    "SELECT namespace,name FROM initialized LIMIT 0",
                    "SELECT account,provider,folder,identity FROM mailbox_identity LIMIT 0",
                    "SELECT account,provider,folder,fetched_at,messages FROM snapshots LIMIT 0",
                ] {
                    connection.query(sql, ()).await.map_err(storage)?;
                }
            }
            // Added after version 1 shipped; a cache table needs no migration of its own.
            connection
                .execute_batch(CONTENT_TABLE)
                .await
                .map_err(storage)?;
            Ok(connection)
        })?;
        Ok(Self {
            path: Arc::new(path),
            connection: Arc::new(Mutex::new(connection)),
        })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub(crate) fn connection(&self) -> Result<MutexGuard<'_, turso::Connection>, Error> {
        self.connection.lock().map_err(storage)
    }
    /// Read a record of the expected schema version. Removed records stay tombstoned.
    pub fn get_record(
        &self,
        namespace: &str,
        name: &str,
        version: u32,
    ) -> Result<Option<Vec<u8>>, Error> {
        validate(namespace, name, version, &[])?;
        let connection = self.connection()?;
        futures_executor::block_on(async {
            let mut rows = connection
                .query(
                    "SELECT version,value FROM records WHERE namespace=?1 AND name=?2",
                    [namespace, name],
                )
                .await
                .map_err(storage)?;
            let Some(row) = rows.next().await.map_err(storage)? else {
                drop(rows);
                let mut initialized = connection
                    .query(
                        "SELECT 1 FROM initialized WHERE namespace=?1 AND name=?2",
                        [namespace, name],
                    )
                    .await
                    .map_err(storage)?;
                if initialized.next().await.map_err(storage)?.is_some() {
                    return Err(storage("Authoritative record is missing"));
                }
                return Ok(None);
            };
            if row.get::<i64>(0).map_err(storage)? != i64::from(version) {
                return Err(storage("Unsupported record version"));
            }
            let value = row.get::<Option<Vec<u8>>>(1).map_err(storage)?;
            if value.as_ref().is_some_and(|v| v.len() > MAX_RECORD_BYTES) {
                return Err(storage("Record exceeds its size limit"));
            }
            Ok(value)
        })
    }
    /// Insert once, including when the default is empty. Never resurrect a removed record.
    pub fn initialize_record(
        &self,
        namespace: &str,
        name: &str,
        version: u32,
        value: &[u8],
    ) -> Result<Vec<u8>, Error> {
        validate(namespace, name, version, value)?;
        if !self.contains_record(namespace, name)? {
            let mut connection = self.connection()?;
            futures_executor::block_on(async {
                let transaction = connection.transaction().await.map_err(storage)?;
                transaction.execute("INSERT OR IGNORE INTO records(namespace,name,version,value) SELECT ?1,?2,?3,?4 WHERE NOT EXISTS (SELECT 1 FROM initialized WHERE namespace=?1 AND name=?2)",turso::params![namespace,name,i64::from(version),value]).await.map_err(storage)?;
                transaction
                    .execute(
                        "INSERT OR IGNORE INTO initialized(namespace,name) VALUES (?1,?2)",
                        [namespace, name],
                    )
                    .await
                    .map_err(storage)?;
                transaction.commit().await.map_err(storage)
            })?;
        }
        self.get_record(namespace, name, version)?
            .ok_or_else(|| storage("Authoritative record was removed"))
    }
    pub fn set_record(
        &self,
        namespace: &str,
        name: &str,
        version: u32,
        value: &[u8],
    ) -> Result<(), Error> {
        validate(namespace, name, version, value)?;
        self.write_record(namespace, name, version, Some(value))
    }
    pub fn remove_record(&self, namespace: &str, name: &str, version: u32) -> Result<(), Error> {
        validate(namespace, name, version, &[])?;
        self.write_record(namespace, name, version, None)
    }
    /// Delete local account data. Safe to retry after a partial removal.
    pub fn remove_account_data(&self, account: &str) -> Result<(), Error> {
        let namespace = format!("account:{account}");
        let mut connection = self.connection()?;
        futures_executor::block_on(async {
            let transaction = connection.transaction().await.map_err(storage)?;
            for sql in [
                "DELETE FROM snapshots WHERE account=?1",
                "DELETE FROM mailbox_identity WHERE account=?1",
                "DELETE FROM message_content WHERE account=?1",
            ] {
                transaction.execute(sql, [account]).await.map_err(storage)?;
            }
            // Keep initialization markers so legacy files cannot resurrect deleted data.
            transaction
                .execute(
                    "UPDATE records SET value=NULL WHERE namespace=?1",
                    [namespace.as_str()],
                )
                .await
                .map_err(storage)?;
            transaction.commit().await.map_err(storage)
        })
    }
    fn write_record(
        &self,
        namespace: &str,
        name: &str,
        version: u32,
        value: Option<&[u8]>,
    ) -> Result<(), Error> {
        let mut connection = self.connection()?;
        futures_executor::block_on(async {
            let transaction = connection.transaction().await.map_err(storage)?;
            let mut rows = transaction
                .query(
                    "SELECT version FROM records WHERE namespace=?1 AND name=?2",
                    [namespace, name],
                )
                .await
                .map_err(storage)?;
            let row = rows.next().await.map_err(storage)?;
            if let Some(row) = &row
                && row.get::<i64>(0).map_err(storage)? != i64::from(version)
            {
                return Err(storage("Unsupported record version"));
            }
            drop(rows);
            if row.is_none() {
                let mut initialized = transaction
                    .query(
                        "SELECT 1 FROM initialized WHERE namespace=?1 AND name=?2",
                        [namespace, name],
                    )
                    .await
                    .map_err(storage)?;
                if initialized.next().await.map_err(storage)?.is_some() {
                    return Err(storage("Authoritative record is missing"));
                }
            }
            transaction.execute("INSERT OR REPLACE INTO records(namespace,name,version,value) VALUES (?1,?2,?3,?4)",turso::params![namespace,name,i64::from(version),value]).await.map_err(storage)?;
            transaction
                .execute(
                    "INSERT OR IGNORE INTO initialized(namespace,name) VALUES (?1,?2)",
                    [namespace, name],
                )
                .await
                .map_err(storage)?;
            transaction.commit().await.map_err(storage)
        })
    }
    pub(crate) fn contains_record(&self, namespace: &str, name: &str) -> Result<bool, Error> {
        let connection = self.connection()?;
        futures_executor::block_on(async {
            Ok(connection
                .query(
                    "SELECT 1 FROM records WHERE namespace=?1 AND name=?2 UNION ALL SELECT 1 FROM initialized WHERE namespace=?1 AND name=?2",
                    [namespace, name],
                )
                .await
                .map_err(storage)?
                .next()
                .await
                .map_err(storage)?
                .is_some())
        })
    }
}
fn validate(namespace: &str, name: &str, version: u32, value: &[u8]) -> Result<(), Error> {
    if namespace.is_empty()
        || namespace.len() > 256
        || name.is_empty()
        || name.len() > 256
        || namespace.chars().chain(name.chars()).any(char::is_control)
        || version == 0
        || value.len() > MAX_RECORD_BYTES
    {
        return Err(storage("Invalid or oversized database record"));
    }
    Ok(())
}
pub(crate) fn private_directory(directory: &Path) -> Result<(), Error> {
    fs::create_dir_all(directory).map_err(storage)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let metadata = fs::symlink_metadata(directory).map_err(storage)?;
        if !metadata.is_dir() || metadata.uid() != rustix::process::getuid().as_raw() {
            return Err(storage("Data directory must be owned by the current user"));
        }
        fs::set_permissions(directory, fs::Permissions::from_mode(0o700)).map_err(storage)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn removed_missing_and_future_records_never_reinitialize() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();
        assert_eq!(
            db.initialize_record("security", "vault", 1, b"first")
                .unwrap(),
            b"first"
        );
        assert_eq!(
            db.initialize_record("security", "vault", 1, b"stale")
                .unwrap(),
            b"first"
        );
        futures_executor::block_on(db.connection().unwrap().execute("DELETE FROM records", ()))
            .unwrap();
        assert!(
            db.initialize_record("security", "vault", 1, b"stale")
                .is_err()
        );
        futures_executor::block_on(db.connection().unwrap().execute(
            "INSERT INTO records(namespace,name,version,value) VALUES ('security','vault',2,?1)",
            [b"future".as_slice()],
        ))
        .unwrap();
        assert!(db.set_record("security", "vault", 1, b"old").is_err());
        assert!(db.remove_record("security", "vault", 1).is_err());
        assert_eq!(
            db.get_record("security", "vault", 2).unwrap().unwrap(),
            b"future"
        );
        assert!(db.get_record("security", "vault", 1).is_err());
        db.remove_record("security", "vault", 2).unwrap();
        assert!(
            db.initialize_record("security", "vault", 2, b"stale")
                .is_err()
        );
        assert!(
            db.set_record("x", "y", 1, &vec![0; MAX_RECORD_BYTES + 1])
                .is_err()
        );
    }
    #[test]
    fn failed_initialization_has_no_marker_and_can_retry() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();
        futures_executor::block_on(db.connection().unwrap().execute("PRAGMA query_only=1", ()))
            .unwrap();
        assert!(db.initialize_record("x", "y", 1, b"first").is_err());
        assert!(!db.contains_record("x", "y").unwrap());
        futures_executor::block_on(db.connection().unwrap().execute("PRAGMA query_only=0", ()))
            .unwrap();
        assert_eq!(
            db.initialize_record("x", "y", 1, b"retry").unwrap(),
            b"retry"
        );
    }
}
