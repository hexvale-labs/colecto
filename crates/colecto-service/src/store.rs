//! Service data in the shared Turso database: whole documents as records in the `service`
//! namespace, and owner settings as one row per changed key. Nothing is written until it
//! changes, so defaults always come from the running build.
use colecto_core::Database;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[cfg(test)]
const NAMESPACE: &str = "service";

/// Read a stored document, or `default` when it was never written.
#[cfg(test)]
pub(crate) fn open<T>(
    database: &Database,
    name: &str,
    default: impl FnOnce() -> T,
    parse: impl Fn(&[u8]) -> Result<T, String>,
) -> Result<T, String> {
    match database
        .get_record(NAMESPACE, name)
        .map_err(|e| e.to_string())?
    {
        Some(bytes) => parse(&bytes),
        None => Ok(default()),
    }
}

#[cfg(test)]
pub(crate) fn save<T: Serialize>(database: &Database, name: &str, value: &T) -> Result<(), String> {
    let bytes = serde_json::to_vec(value).map_err(|e| e.to_string())?;
    database
        .set_record(NAMESPACE, name, &bytes)
        .map_err(|e| e.to_string())
}

/// The setting keys of `T`: dotted paths to its leaves, such as `notifications.position`.
/// A field whose default is an object with fields (a nested struct) is split into its
/// fields; anything else, including maps that start empty, is one setting.
fn leaves(guide: &Value, value: &Value, prefix: &str, out: &mut BTreeMap<String, Value>) {
    match (guide, value) {
        (Value::Object(fields), Value::Object(values)) if !fields.is_empty() => {
            for (key, field) in fields {
                let path = if prefix.is_empty() {
                    key.clone()
                } else {
                    format!("{prefix}.{key}")
                };
                leaves(field, values.get(key).unwrap_or(&Value::Null), &path, out);
            }
        }
        _ => {
            out.insert(prefix.into(), value.clone());
        }
    }
}
fn keys<T: Serialize + Default>(value: &T) -> BTreeMap<String, Value> {
    let guide = serde_json::to_value(T::default()).expect("settings serialize");
    let mut out = BTreeMap::new();
    leaves(
        &guide,
        &serde_json::to_value(value).expect("settings serialize"),
        "",
        &mut out,
    );
    out
}
fn set_path(target: &mut Value, path: &str, value: Value) {
    let mut node = target;
    let mut parts = path.split('.').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            node[part] = value;
            return;
        }
        node = &mut node[part];
    }
}

/// Settings in `scope`: the build's defaults with the owner's changes on top, and the keys
/// they changed. A stored key the build no longer knows, or a value that no longer
/// validates, is skipped with a warning instead of keeping Colecto from starting.
pub(crate) fn load_settings<T: Serialize + DeserializeOwned + Default>(
    database: &Database,
    scope: &str,
    validate: impl Fn(&T) -> Result<(), String>,
) -> Result<(T, BTreeSet<String>), String> {
    let known = keys(&T::default());
    let mut merged = serde_json::to_value(T::default()).map_err(|e| e.to_string())?;
    let mut changed = BTreeSet::new();
    for (key, text) in database.settings(scope).map_err(|e| e.to_string())? {
        let accepted = known.contains_key(&key)
            && serde_json::from_str::<Value>(&text).is_ok_and(|value| {
                let mut candidate = merged.clone();
                set_path(&mut candidate, &key, value);
                let valid = serde_json::from_value::<T>(candidate.clone())
                    .is_ok_and(|parsed| validate(&parsed).is_ok());
                if valid {
                    merged = candidate;
                }
                valid
            });
        if accepted {
            changed.insert(key);
        } else {
            tracing::warn!(
                scope,
                key,
                "Ignoring a stored setting this version cannot use"
            );
        }
    }
    let value = serde_json::from_value(merged).map_err(|e| e.to_string())?;
    Ok((value, changed))
}

/// Store the keys that differ between `before` and `after`, and forget the `reset` keys so
/// they follow the default again.
pub(crate) fn save_settings<T: Serialize + Default>(
    database: &Database,
    scope: &str,
    before: &T,
    after: &T,
    reset: &[String],
) -> Result<(), String> {
    let (before, after) = (keys(before), keys(after));
    if let Some(key) = reset.iter().find(|key| !after.contains_key(*key)) {
        return Err(format!("Unknown setting: {key}"));
    }
    let mut changes: Vec<(String, Option<String>)> = after
        .iter()
        .filter(|(key, value)| before.get(*key) != Some(value) && !reset.contains(key))
        .map(|(key, value)| (key.clone(), Some(value.to_string())))
        .collect();
    changes.extend(reset.iter().map(|key| (key.clone(), None)));
    if changes.is_empty() {
        return Ok(());
    }
    database
        .write_settings(scope, &changes)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Serialize, Deserialize, Default, Clone, PartialEq, Debug)]
    #[serde(default)]
    struct Inner {
        size: u8,
        name: Option<String>,
    }
    #[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
    #[serde(default)]
    struct Sample {
        on: bool,
        inner: Inner,
        map: BTreeMap<String, bool>,
    }
    impl Default for Sample {
        fn default() -> Self {
            Self {
                on: true,
                inner: Inner::default(),
                map: BTreeMap::new(),
            }
        }
    }
    fn small(sample: &Sample) -> Result<(), String> {
        (sample.inner.size < 10)
            .then_some(())
            .ok_or("too big".into())
    }

    #[test]
    fn missing_records_read_as_default_without_being_written() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::open(dir.path()).unwrap();
        let number = |b: &[u8]| serde_json::from_slice::<u32>(b).map_err(|e| e.to_string());
        assert_eq!(open(&database, "sample", || 7, number), Ok(7));
        assert_eq!(database.get_record(NAMESPACE, "sample").unwrap(), None);
        save(&database, "sample", &9u32).unwrap();
        assert_eq!(open(&database, "sample", || 0, number), Ok(9));
        database
            .set_record(NAMESPACE, "sample", b"corrupt")
            .unwrap();
        assert!(open(&database, "sample", || 0, number).is_err());
    }

    #[test]
    fn only_changed_settings_are_stored_and_reset_follows_the_default() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::open(dir.path()).unwrap();
        let (before, changed) = load_settings::<Sample>(&database, "s", small).unwrap();
        assert_eq!((before.clone(), changed.len()), (Sample::default(), 0));

        let mut after = before.clone();
        after.inner.size = 3;
        after.map.insert("a".into(), false);
        save_settings(&database, "s", &before, &after, &[]).unwrap();
        assert_eq!(
            database.settings("s").unwrap(),
            vec![
                ("inner.size".to_string(), "3".to_string()),
                ("map".to_string(), r#"{"a":false}"#.to_string())
            ]
        );
        let (loaded, changed) = load_settings::<Sample>(&database, "s", small).unwrap();
        assert_eq!(loaded, after);
        assert!(changed.contains("inner.size") && !changed.contains("on"));

        // Choosing the default value on purpose is kept; reset forgets the key.
        let mut explicit = loaded.clone();
        explicit.inner.size = 0;
        save_settings(&database, "s", &loaded, &explicit, &[]).unwrap();
        assert!(
            load_settings::<Sample>(&database, "s", small)
                .unwrap()
                .1
                .contains("inner.size")
        );
        save_settings(&database, "s", &explicit, &explicit, &["inner.size".into()]).unwrap();
        assert!(
            !load_settings::<Sample>(&database, "s", small)
                .unwrap()
                .1
                .contains("inner.size")
        );
        assert!(save_settings(&database, "s", &explicit, &explicit, &["nope".into()]).is_err());
    }

    #[test]
    fn unknown_and_invalid_stored_settings_fall_back_to_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let database = Database::open(dir.path()).unwrap();
        database
            .write_settings(
                "s",
                &[
                    ("gone".into(), Some("1".into())),
                    ("inner.size".into(), Some("99".into())),
                    ("on".into(), Some("\"not a bool\"".into())),
                    ("inner.name".into(), Some("\"kept\"".into())),
                ],
            )
            .unwrap();
        let (loaded, changed) = load_settings::<Sample>(&database, "s", small).unwrap();
        assert_eq!(loaded.inner.size, 0);
        assert!(loaded.on);
        assert_eq!(loaded.inner.name.as_deref(), Some("kept"));
        assert_eq!(changed, BTreeSet::from(["inner.name".to_string()]));
    }
}
