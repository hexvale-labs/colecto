//! Host policy and preferences live in the shared Turso database, never in JSON files.
//! Missing records initialize from defaults; old JSON files are ignored.
use colecto_core::Database;
use serde::Serialize;

const NAMESPACE: &str = "host";
const VERSION: u32 = 1;

/// Read the canonical record or initialize it from `default`, validating the stored bytes.
pub(crate) fn open<T: Serialize>(
    database: &Database,
    name: &str,
    default: impl FnOnce() -> T,
    parse: impl Fn(&[u8]) -> Result<T, String>,
) -> Result<T, String> {
    if let Some(bytes) = database
        .get_record(NAMESPACE, name, VERSION)
        .map_err(|e| e.to_string())?
    {
        return parse(&bytes);
    }
    let value = default();
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
    #[test]
    fn missing_record_initializes_from_default_and_saved_record_is_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::open(dir.path()).unwrap();
        std::fs::write(dir.path().join("sample.json"), "corrupt").unwrap();
        assert_eq!(open(&database, "sample", || 7, number), Ok(7));
        assert_eq!(open(&database, "sample", || 0, number), Ok(7));
        save(&database, "sample", &9u32).unwrap();
        assert_eq!(open(&database, "sample", || 0, number), Ok(9));
    }

    #[test]
    fn invalid_canonical_bytes_fail() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::open(dir.path()).unwrap();
        database
            .set_record(NAMESPACE, "sample", VERSION, b"corrupt")
            .unwrap();
        assert!(open(&database, "sample", || 0, number).is_err());
    }
}
