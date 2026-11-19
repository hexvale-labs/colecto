//! Private, shared application storage. Callers validate their own serialized formats.
use crate::{Error, storage};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
};

const MAX_RECORD_BYTES: usize = 32_000_000;
/// Marks a database as Colecto's (`PRAGMA application_id`, the bytes "COLE"). A database
/// without it, including one from Colecto alpha, is refused rather than guessed at.
pub const APPLICATION_ID: i64 = 0x434F_4C45;
/// Schema steps in order. `PRAGMA user_version` counts the steps already applied. A step
/// that has shipped never changes; a schema change adds the next file.
const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_initial.sql")];
/// The schema version this build writes.
pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;

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
            migrate(&connection, &path).await?;
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
    /// Move the write-ahead log into `colecto.db`, so the file alone holds every change.
    pub fn checkpoint(&self) -> Result<(), Error> {
        futures_executor::block_on(checkpoint(&*self.connection()?))
    }
    pub(crate) fn connection(&self) -> Result<MutexGuard<'_, turso::Connection>, Error> {
        self.connection.lock().map_err(storage)
    }
    /// Read a record, or `None` when it was never written.
    pub fn get_record(&self, namespace: &str, name: &str) -> Result<Option<Vec<u8>>, Error> {
        validate(namespace, name, &[])?;
        let connection = self.connection()?;
        futures_executor::block_on(async {
            let mut rows = connection
                .query(
                    "SELECT value FROM records WHERE namespace=?1 AND name=?2",
                    [namespace, name],
                )
                .await
                .map_err(storage)?;
            let Some(row) = rows.next().await.map_err(storage)? else {
                return Ok(None);
            };
            let value = row.get::<Vec<u8>>(0).map_err(storage)?;
            if value.len() > MAX_RECORD_BYTES {
                return Err(storage("Record exceeds its size limit"));
            }
            Ok(Some(value))
        })
    }
    /// Write `value` unless the record exists, then return what is stored.
    pub fn initialize_record(
        &self,
        namespace: &str,
        name: &str,
        value: &[u8],
    ) -> Result<Vec<u8>, Error> {
        validate(namespace, name, value)?;
        {
            let connection = self.connection()?;
            futures_executor::block_on(connection.execute(
                "INSERT OR IGNORE INTO records(namespace,name,value) VALUES (?1,?2,?3)",
                turso::params![namespace, name, value],
            ))
            .map_err(storage)?;
        }
        self.get_record(namespace, name)?
            .ok_or_else(|| storage("Record was not written"))
    }
    pub fn set_record(&self, namespace: &str, name: &str, value: &[u8]) -> Result<(), Error> {
        validate(namespace, name, value)?;
        let connection = self.connection()?;
        futures_executor::block_on(connection.execute(
            "INSERT OR REPLACE INTO records(namespace,name,value) VALUES (?1,?2,?3)",
            turso::params![namespace, name, value],
        ))
        .map_err(storage)?;
        Ok(())
    }
    pub fn remove_record(&self, namespace: &str, name: &str) -> Result<(), Error> {
        validate(namespace, name, &[])?;
        let connection = self.connection()?;
        futures_executor::block_on(connection.execute(
            "DELETE FROM records WHERE namespace=?1 AND name=?2",
            [namespace, name],
        ))
        .map_err(storage)?;
        Ok(())
    }
    /// Every setting the owner changed in `scope`, as `(key, JSON value)`.
    pub fn settings(&self, scope: &str) -> Result<Vec<(String, String)>, Error> {
        validate(scope, "-", &[])?;
        let connection = self.connection()?;
        futures_executor::block_on(async {
            let mut rows = connection
                .query(
                    "SELECT key,value FROM settings WHERE scope=?1 ORDER BY key",
                    [scope],
                )
                .await
                .map_err(storage)?;
            let mut settings = Vec::new();
            while let Some(row) = rows.next().await.map_err(storage)? {
                settings.push((
                    row.get::<String>(0).map_err(storage)?,
                    row.get::<String>(1).map_err(storage)?,
                ));
            }
            Ok(settings)
        })
    }
    /// Store changed settings and forget reset ones (`None`) in one transaction.
    pub fn write_settings(
        &self,
        scope: &str,
        changes: &[(String, Option<String>)],
    ) -> Result<(), Error> {
        for (key, value) in changes {
            validate(
                scope,
                key,
                value.as_deref().map(str::as_bytes).unwrap_or_default(),
            )?;
        }
        let mut connection = self.connection()?;
        futures_executor::block_on(async {
            let transaction = connection.transaction().await.map_err(storage)?;
            for (key, value) in changes {
                match value {
                    Some(value) => transaction
                        .execute(
                            "INSERT OR REPLACE INTO settings(scope,key,value) VALUES (?1,?2,?3)",
                            [scope, key.as_str(), value.as_str()],
                        )
                        .await
                        .map_err(storage)?,
                    None => transaction
                        .execute(
                            "DELETE FROM settings WHERE scope=?1 AND key=?2",
                            [scope, key.as_str()],
                        )
                        .await
                        .map_err(storage)?,
                };
            }
            transaction.commit().await.map_err(storage)
        })
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
            transaction
                .execute(
                    "DELETE FROM records WHERE namespace=?1",
                    [namespace.as_str()],
                )
                .await
                .map_err(storage)?;
            transaction.commit().await.map_err(storage)
        })
    }
}
async fn pragma(connection: &turso::Connection, name: &str) -> Result<i64, Error> {
    let mut rows = connection
        .query(&format!("PRAGMA {name}"), ())
        .await
        .map_err(storage)?;
    rows.next()
        .await
        .map_err(storage)?
        .ok_or_else(|| storage("Missing database pragma"))?
        .get::<i64>(0)
        .map_err(storage)
}
/// Bring the schema up to [`SCHEMA_VERSION`], one transaction per step, after copying the
/// database aside. Refuses databases that are not Colecto's or come from a newer build.
async fn migrate(connection: &turso::Connection, path: &Path) -> Result<(), Error> {
    let version = pragma(connection, "user_version").await?;
    let id = pragma(connection, "application_id").await?;
    let mut tables = connection
        .query("SELECT count(*) FROM sqlite_schema", ())
        .await
        .map_err(storage)?;
    let empty = tables
        .next()
        .await
        .map_err(storage)?
        .ok_or_else(|| storage("Missing database schema"))?
        .get::<i64>(0)
        .map_err(storage)?
        == 0;
    drop(tables);
    if !empty && id != APPLICATION_ID {
        return Err(storage(
            "colecto.db was made by Colecto alpha or another program. Move it out of the data directory to start fresh.",
        ));
    }
    if version > SCHEMA_VERSION {
        return Err(storage(
            "colecto.db was made by a newer Colecto. Update Colecto to open it.",
        ));
    }
    if version == SCHEMA_VERSION {
        return Ok(());
    }
    if !empty {
        backup(connection, path, version).await?;
    }
    for (step, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
        let next = step as i64 + 1;
        connection
            .execute_batch(&format!(
                "BEGIN; {sql}\nPRAGMA application_id={APPLICATION_ID}; PRAGMA user_version={next}; COMMIT;"
            ))
            .await
            .map_err(|error| {
                storage(format!("Database migration {next} failed: {error}"))
            })?;
        tracing::info!(version = next, "Migrated the database");
    }
    Ok(())
}
/// Move everything in the write-ahead log into `colecto.db` itself.
async fn checkpoint(connection: &turso::Connection) -> Result<(), Error> {
    let mut rows = connection
        .query("PRAGMA wal_checkpoint(TRUNCATE)", ())
        .await
        .map_err(storage)?;
    while rows.next().await.map_err(storage)?.is_some() {}
    Ok(())
}
/// Copy the database to `colecto.db.before-v<N>` before migrating it, keeping one copy.
async fn backup(connection: &turso::Connection, path: &Path, version: i64) -> Result<(), Error> {
    checkpoint(connection).await?;
    let directory = path
        .parent()
        .ok_or_else(|| storage("Invalid database path"))?;
    for entry in fs::read_dir(directory).map_err(storage)?.flatten() {
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with("colecto.db.before-v")
        {
            fs::remove_file(entry.path()).map_err(storage)?;
        }
    }
    fs::copy(
        path,
        directory.join(format!("colecto.db.before-v{version}")),
    )
    .map_err(storage)?;
    Ok(())
}
fn validate(namespace: &str, name: &str, value: &[u8]) -> Result<(), Error> {
    if namespace.is_empty()
        || namespace.len() > 256
        || name.is_empty()
        || name.len() > 256
        || namespace.chars().chain(name.chars()).any(char::is_control)
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
    fn user_version(db: &Database) -> i64 {
        futures_executor::block_on(pragma(&db.connection().unwrap(), "user_version")).unwrap()
    }
    #[test]
    fn new_database_is_marked_and_migrated() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();
        assert_eq!(user_version(&db), SCHEMA_VERSION);
        assert_eq!(
            futures_executor::block_on(pragma(&db.connection().unwrap(), "application_id"))
                .unwrap(),
            APPLICATION_ID
        );
        drop(db);
        // Reopening an up-to-date database neither migrates nor copies it.
        Database::open(dir.path()).unwrap();
        assert!(!dir.path().join("colecto.db.before-v1").exists());
    }
    #[test]
    fn foreign_and_newer_databases_are_refused() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();
        futures_executor::block_on(
            db.connection()
                .unwrap()
                .execute_batch(&format!("PRAGMA user_version={};", SCHEMA_VERSION + 1)),
        )
        .unwrap();
        drop(db);
        let error = Database::open(dir.path()).err().unwrap().to_string();
        assert!(error.contains("newer Colecto"), "{error}");

        // An alpha database: tables, but no application ID.
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();
        futures_executor::block_on(
            db.connection()
                .unwrap()
                .execute_batch("PRAGMA application_id=0;"),
        )
        .unwrap();
        drop(db);
        let error = Database::open(dir.path()).err().unwrap().to_string();
        assert!(error.contains("Colecto alpha"), "{error}");
    }
    #[test]
    fn an_older_database_is_copied_before_migrating() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();
        db.set_record("x", "y", b"kept").unwrap();
        // Pretend no step ran yet. Step 1 then fails on the existing tables, after the copy.
        futures_executor::block_on(
            db.connection()
                .unwrap()
                .execute_batch("PRAGMA user_version=0;"),
        )
        .unwrap();
        drop(db);
        let error = Database::open(dir.path()).err().unwrap().to_string();
        assert!(dir.path().join("colecto.db.before-v0").exists(), "{error}");
        let copy = tempfile::tempdir().unwrap();
        fs::copy(
            dir.path().join("colecto.db.before-v0"),
            copy.path().join("colecto.db"),
        )
        .unwrap();
        futures_executor::block_on(async {
            let database =
                turso::Builder::new_local(copy.path().join("colecto.db").to_str().unwrap())
                    .build()
                    .await
                    .unwrap();
            let connection = database.connect().unwrap();
            let mut rows = connection
                .query("SELECT value FROM records WHERE name='y'", ())
                .await
                .unwrap();
            let row = rows.next().await.unwrap().unwrap();
            assert_eq!(row.get::<Vec<u8>>(0).unwrap(), b"kept");
        });
    }
    #[test]
    fn records_and_settings_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();
        assert_eq!(db.get_record("x", "y").unwrap(), None);
        assert_eq!(db.initialize_record("x", "y", b"first").unwrap(), b"first");
        assert_eq!(db.initialize_record("x", "y", b"second").unwrap(), b"first");
        db.set_record("x", "y", b"third").unwrap();
        assert_eq!(db.get_record("x", "y").unwrap().unwrap(), b"third");
        db.remove_record("x", "y").unwrap();
        assert_eq!(db.get_record("x", "y").unwrap(), None);
        assert!(
            db.set_record("x", "y", &vec![0; MAX_RECORD_BYTES + 1])
                .is_err()
        );

        db.write_settings(
            "core",
            &[
                ("a".into(), Some("1".into())),
                ("b".into(), Some("\"two\"".into())),
            ],
        )
        .unwrap();
        db.write_settings("core", &[("a".into(), None)]).unwrap();
        assert_eq!(
            db.settings("core").unwrap(),
            vec![("b".to_string(), "\"two\"".to_string())]
        );
        assert!(db.settings("ai").unwrap().is_empty());
    }
}
