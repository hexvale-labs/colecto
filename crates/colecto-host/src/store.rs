//! Host policy and preferences live in the shared Turso database, never in JSON files.
//! A valid legacy `<name>.json` imports once; the file is then left untouched as rollback evidence.
use colecto_core::Database;
use serde::Serialize;
use std::path::Path;

const NAMESPACE: &str = "host";
const VERSION: u32 = 1;

pub(crate) struct Legacy {
    pub max_bytes: u64,
    /// Files that held secrets or delivery content had to be owner-only.
    pub private: bool,
}

/// Read the canonical record, importing a legacy file (when one existed) or `default` the first time.
/// `parse` validates every source; an invalid legacy file fails startup visibly.
pub(crate) fn open<T: Serialize>(
    database: &Database,
    directory: &Path,
    name: &str,
    legacy: Option<Legacy>,
    default: impl FnOnce() -> T,
    parse: impl Fn(&[u8]) -> Result<T, String>,
) -> Result<T, String> {
    if let Some(bytes) = database
        .get_record(NAMESPACE, name, VERSION)
        .map_err(|e| e.to_string())?
    {
        return parse(&bytes);
    }
    let path = directory.join(format!("{name}.json"));
    let value = match legacy.map(|legacy| (legacy, std::fs::symlink_metadata(&path))) {
        None => default(),
        Some((legacy, Ok(meta))) => {
            if !meta.is_file() || meta.len() > legacy.max_bytes {
                return Err(format!("Invalid legacy {name} file"));
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if legacy.private && meta.permissions().mode() & 0o077 != 0 {
                    return Err(format!("Legacy {name} file must be private"));
                }
            }
            parse(&std::fs::read(&path).map_err(|_| format!("Cannot read legacy {name} file"))?)?
        }
        Some((_, Err(e))) if e.kind() == std::io::ErrorKind::NotFound => default(),
        Some(_) => return Err(format!("Cannot read legacy {name} file")),
    };
    let bytes = serde_json::to_vec(&value).map_err(|e| e.to_string())?;
    parse(
        &database
            .initialize_record(NAMESPACE, name, VERSION, &bytes)
            .map_err(|e| e.to_string())?,
    )
}

pub(crate) fn save<T: Serialize>(database: &Database, name: &str, value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    database
        .set_record(NAMESPACE, name, VERSION, &bytes)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(bytes: &[u8]) -> Result<u32, String> {
        serde_json::from_slice(bytes).map_err(|_| "invalid".to_string())
    }
    fn legacy() -> Option<Legacy> {
        Some(Legacy {
            max_bytes: 64,
            private: false,
        })
    }

    #[test]
    fn legacy_json_imports_once_and_is_never_written_again() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::open(dir.path()).unwrap();
        std::fs::write(dir.path().join("sample.json"), "7").unwrap();
        assert_eq!(
            open(&database, dir.path(), "sample", legacy(), || 0, number),
            Ok(7)
        );
        save(&database, "sample", &9u32).unwrap();
        // Later edits to the old file never override canonical state.
        std::fs::write(dir.path().join("sample.json"), "1").unwrap();
        assert_eq!(
            open(&database, dir.path(), "sample", legacy(), || 0, number),
            Ok(9)
        );
        assert_eq!(
            std::fs::read_to_string(dir.path().join("sample.json")).unwrap(),
            "1"
        );
    }

    #[test]
    fn invalid_legacy_json_fails_without_initializing() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::open(dir.path()).unwrap();
        std::fs::write(dir.path().join("sample.json"), "corrupt").unwrap();
        assert!(open(&database, dir.path(), "sample", legacy(), || 0, number).is_err());
        std::fs::remove_file(dir.path().join("sample.json")).unwrap();
        assert_eq!(
            open(&database, dir.path(), "sample", legacy(), || 3, number),
            Ok(3)
        );
    }
}
