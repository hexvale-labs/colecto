//! Private, shared application storage. Callers validate their own serialized formats.
use crate::{Error, storage};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{Arc, Mutex, MutexGuard},
    time::{SystemTime, UNIX_EPOCH},
};

const MAX_RECORD_BYTES: usize = 32_000_000;
const MAX_BACKUPS: usize = 5;
/// Marks a database as Colecto's (`PRAGMA application_id`, the bytes "COLE").
/// A foreign database without it is refused rather than guessed at.
pub const APPLICATION_ID: i64 = 0x434F_4C45;
/// Schema steps in order. `PRAGMA user_version` counts the steps already applied. A step
/// that has shipped never changes; a schema change adds the next file.
const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_initial.sql")];
/// The schema version this build writes.
pub const SCHEMA_VERSION: i64 = MIGRATIONS.len() as i64;
pub use turso::Value;

/// One result row of [`Database::query`].
pub struct Row(Vec<Value>);
impl Row {
    fn value(&self, index: usize) -> Result<&Value, Error> {
        self.0
            .get(index)
            .ok_or_else(|| storage("Missing database column"))
    }
    pub fn text(&self, index: usize) -> Result<String, Error> {
        self.optional_text(index)?
            .ok_or_else(|| storage("Unexpected empty database column"))
    }
    pub fn optional_text(&self, index: usize) -> Result<Option<String>, Error> {
        match self.value(index)? {
            Value::Null => Ok(None),
            Value::Text(text) => Ok(Some(text.clone())),
            _ => Err(storage("Expected a text database column")),
        }
    }
    pub fn integer(&self, index: usize) -> Result<i64, Error> {
        self.optional_integer(index)?
            .ok_or_else(|| storage("Unexpected empty database column"))
    }
    pub fn optional_integer(&self, index: usize) -> Result<Option<i64>, Error> {
        match self.value(index)? {
            Value::Null => Ok(None),
            Value::Integer(number) => Ok(Some(*number)),
            _ => Err(storage("Expected an integer database column")),
        }
    }
    pub fn blob(&self, index: usize) -> Result<Vec<u8>, Error> {
        match self.value(index)? {
            Value::Blob(bytes) => Ok(bytes.clone()),
            _ => Err(storage("Expected a blob database column")),
        }
    }
}

#[derive(Clone)]
pub struct Database {
    path: Arc<PathBuf>,
    connection: Arc<Mutex<turso::Connection>>,
}
impl Database {
    pub fn open(directory: impl AsRef<Path>) -> Result<Self, Error> {
        Self::open_with_migrations(directory, MIGRATIONS)
    }
    fn open_with_migrations(
        directory: impl AsRef<Path>,
        migrations: &[&str],
    ) -> Result<Self, Error> {
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
            migrate(&connection, &path, migrations).await?;
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
    /// Run one statement against the schema in `migrations/` and return its rows.
    /// For internal core and service layers only; untrusted clients never send SQL.
    pub fn query(&self, sql: &str, params: Vec<Value>) -> Result<Vec<Row>, Error> {
        let connection = self.connection()?;
        futures_executor::block_on(async {
            let mut rows = connection.query(sql, params).await.map_err(storage)?;
            let mut out = Vec::new();
            while let Some(row) = rows.next().await.map_err(storage)? {
                out.push(Row((0..row.column_count())
                    .map(|index| row.get_value(index))
                    .collect::<Result<_, _>>()
                    .map_err(storage)?));
            }
            Ok(out)
        })
    }
    /// Run statements in one transaction: all of them apply, or none.
    pub fn write(&self, statements: Vec<(&str, Vec<Value>)>) -> Result<(), Error> {
        let mut connection = self.connection()?;
        futures_executor::block_on(async {
            let transaction = connection.transaction().await.map_err(storage)?;
            for (sql, params) in statements {
                transaction.execute(sql, params).await.map_err(storage)?;
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
                "DELETE FROM drafts WHERE account=?1",
                "DELETE FROM labels WHERE account=?1",
                "DELETE FROM label_messages WHERE account=?1",
                "DELETE FROM ai_summaries WHERE account=?1",
                "DELETE FROM ai_autolabel_seen WHERE account=?1",
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

/// Bring the schema up to the latest migration, one transaction per step, after copying
/// the database into `backups/`. Refuses foreign databases or ones from a newer Colecto build.
async fn migrate(
    connection: &turso::Connection,
    path: &Path,
    migrations: &[&str],
) -> Result<(), Error> {
    let target_version = migrations.len() as i64;
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
            "colecto.db was made by another program. Move it out of the data directory to start fresh.",
        ));
    }
    if version > target_version {
        return Err(storage(
            "colecto.db was made by a newer Colecto. Update Colecto to open it.",
        ));
    }
    if version == target_version {
        return Ok(());
    }
    if !empty {
        backup(connection, path, version).await?;
    }
    for (step, sql) in migrations.iter().enumerate().skip(version as usize) {
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

/// Copy the database to `backups/v<N>-<ts>.db` before migrating it, keeping the 5 newest backups.
async fn backup(connection: &turso::Connection, path: &Path, version: i64) -> Result<(), Error> {
    checkpoint(connection).await?;
    let directory = path
        .parent()
        .ok_or_else(|| storage("Invalid database path"))?;
    let backups = directory.join("backups");
    private_directory(&backups)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let mut candidate = backups.join(format!("v{version}-{stamp:016}.db"));
    let mut suffix = 0u32;
    while candidate.exists() {
        suffix += 1;
        candidate = backups.join(format!("v{version}-{stamp:016}-{suffix}.db"));
    }
    fs::copy(path, &candidate).map_err(storage)?;
    let mut existing: Vec<_> = fs::read_dir(&backups)
        .map_err(storage)?
        .flatten()
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name.starts_with('v') && name.ends_with(".db")
        })
        .collect();
    existing.sort_by_key(|entry| entry.file_name());
    for stale in existing
        .iter()
        .take(existing.len().saturating_sub(MAX_BACKUPS))
    {
        fs::remove_file(stale.path()).map_err(storage)?;
    }
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
        assert!(dir.path().join("colecto.db").exists());
        drop(db);
        // Reopening an up-to-date database neither migrates nor backs up.
        Database::open(dir.path()).unwrap();
        assert!(!dir.path().join("backups").exists());
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

        // A database without Colecto's application ID.
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
        assert!(error.contains("another program"), "{error}");
    }

    #[test]
    fn forward_migration_backs_up_database_and_rolls_back_on_error() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(dir.path()).unwrap();
        db.set_record("x", "y", b"kept").unwrap();
        drop(db);

        // A valid v1 -> v2 migration creates a backup under backups/ and upgrades cleanly.
        let v2_migrations = &[
            MIGRATIONS[0],
            "CREATE TABLE extra_meta(id TEXT PRIMARY KEY, val TEXT NOT NULL);",
        ];
        let db2 = Database::open_with_migrations(dir.path(), v2_migrations).unwrap();
        assert_eq!(user_version(&db2), 2);
        assert_eq!(db2.get_record("x", "y").unwrap().unwrap(), b"kept");
        let backups: Vec<_> = fs::read_dir(dir.path().join("backups"))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(backups.len(), 1);
        assert!(backups[0].file_name().to_string_lossy().starts_with("v1-"));
        drop(db2);

        // Failing v2 -> v3 migration creates a backup, leaves user_version at 2, and rolls back DDL.
        let bad_v3 = &[
            MIGRATIONS[0],
            "CREATE TABLE extra_meta(id TEXT PRIMARY KEY, val TEXT NOT NULL);",
            "CREATE TABLE should_rollback(id TEXT PRIMARY KEY); INVALID SQL SYNTAX HERE;",
        ];
        let err = Database::open_with_migrations(dir.path(), bad_v3)
            .err()
            .unwrap()
            .to_string();
        assert!(err.contains("Database migration 3 failed"), "{err}");
        let db_check = Database::open_with_migrations(dir.path(), v2_migrations).unwrap();
        assert_eq!(user_version(&db_check), 2);
        assert!(
            db_check
                .query(
                    "SELECT name FROM sqlite_schema WHERE name='should_rollback'",
                    vec![]
                )
                .unwrap()
                .is_empty()
        );
        drop(db_check);

        // Trigger repeated migrations to confirm at most MAX_BACKUPS (5) are retained.
        let mut chain: Vec<String> = v2_migrations.iter().map(|s| (*s).to_owned()).collect();
        for step in 3..=9 {
            chain.push(format!("CREATE TABLE step_{step}(id TEXT PRIMARY KEY);"));
            let refs: Vec<&str> = chain.iter().map(String::as_str).collect();
            let db_n = Database::open_with_migrations(dir.path(), &refs).unwrap();
            assert_eq!(user_version(&db_n), step as i64);
        }
        let retained: Vec<_> = fs::read_dir(dir.path().join("backups"))
            .unwrap()
            .flatten()
            .collect();
        assert_eq!(retained.len(), MAX_BACKUPS);
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
