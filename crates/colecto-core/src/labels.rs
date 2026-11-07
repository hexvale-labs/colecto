use super::*;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn labels_bound_storage_and_preserve_corrupt_files() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = MailStore::open(directory.path()).unwrap();
        for index in 0..50 {
            store
                .update_label(&format!("Label {index}"), None, false)
                .unwrap();
        }
        let before = fs::read(directory.path().join("labels.json")).unwrap();
        assert!(store.update_label("Extra", None, false).is_err());
        assert!(store.update_label(&"界".repeat(27), None, false).is_err());
        assert_eq!(
            fs::read(directory.path().join("labels.json")).unwrap(),
            before
        );
        fs::write(directory.path().join("labels.json"), b"invalid metadata").unwrap();
        assert!(store.update_label("Work", None, false).is_err());
        assert_eq!(
            fs::read(directory.path().join("labels.json")).unwrap(),
            b"invalid metadata"
        );
    }
    #[test]
    fn labels_persist_validate_and_stay_in_their_store() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = MailStore::open(directory.path().join("a")).unwrap();
        let id = store.messages()[0].id.clone();
        store.update_label("Work", None, false).unwrap();
        store.update_label(" Work ", Some(&id), false).unwrap();
        assert!(store.labels().unwrap()["Work"].contains(&id));
        assert!(store.update_label("bad\nname", None, false).is_err());
        assert!(store.update_label("Work", Some("missing"), false).is_err());
        drop(store);
        let mut store = MailStore::open(directory.path().join("a")).unwrap();
        assert!(store.labels().unwrap()["Work"].contains(&id));
        assert!(
            MailStore::open(directory.path().join("b"))
                .unwrap()
                .labels()
                .unwrap()
                .is_empty()
        );
        store.update_label("Work", Some(&id), true).unwrap();
        assert!(store.labels().unwrap()["Work"].is_empty());
        store.update_label("Work", None, true).unwrap();
        assert!(store.labels().unwrap().is_empty());
    }
}

pub type Labels = std::collections::BTreeMap<String, Vec<String>>;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedLabels {
    version: u32,
    labels: Labels,
}
fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name == name.trim()
        && name.len() <= 80
        && !name.chars().any(char::is_control)
}
impl MailStore {
    pub fn labels(&self) -> Result<Labels, Error> {
        let path = self.directory.join("labels.json");
        let file = match File::open(path) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Labels::new()),
            Err(e) => return Err(storage(e)),
        };
        if file.metadata().map_err(storage)?.len() > 2_000_000 {
            return Err(Error::Storage("Label store exceeds its size limit".into()));
        }
        let saved: SavedLabels = serde_json::from_reader(file).map_err(storage)?;
        if saved.version != 1
            || saved.labels.len() > 50
            || saved.labels.iter().any(|(name, ids)| {
                !valid_name(name)
                    || ids.len() > 1000
                    || ids.iter().any(|id| {
                        id.is_empty() || id.len() > 1024 || id.chars().any(char::is_control)
                    })
                    || ids.iter().collect::<HashSet<_>>().len() != ids.len()
            })
        {
            return Err(Error::Storage("Invalid label store".into()));
        }
        Ok(saved.labels)
    }
    pub fn update_label(
        &mut self,
        name: &str,
        message_id: Option<&str>,
        remove: bool,
    ) -> Result<Labels, Error> {
        if name.chars().any(char::is_control) || !valid_name(name.trim()) {
            return Err(Error::Invalid(
                "Use a label of 1–80 bytes without control characters".into(),
            ));
        }
        let name = name.trim();
        let mut labels = self.labels()?;
        if let Some(id) = message_id {
            if id.len() > 1024 || id.chars().any(char::is_control) || self.message(id).is_none() {
                return Err(Error::Missing);
            }
            let ids = labels.get_mut(name).ok_or(Error::Missing)?;
            if remove {
                ids.retain(|entry| entry != id);
            } else if !ids.iter().any(|entry| entry == id) {
                if ids.len() >= 1000 {
                    return Err(Error::Invalid(
                        "A label can contain at most 1000 messages".into(),
                    ));
                }
                ids.push(id.to_owned());
            }
        } else if remove {
            labels.remove(name);
        } else if !labels.contains_key(name) {
            if labels.len() >= 50 {
                return Err(Error::Invalid(
                    "An account can have at most 50 labels".into(),
                ));
            }
            labels.insert(name.to_owned(), Vec::new());
        }
        let saved = SavedLabels { version: 1, labels };
        let bytes = serde_json::to_vec(&saved).map_err(storage)?;
        if bytes.len() > 2_000_000 {
            return Err(Error::Invalid("Label store exceeds its size limit".into()));
        }
        let mut temporary = tempfile::NamedTempFile::new_in(&self.directory).map_err(storage)?;
        temporary.write_all(&bytes).map_err(storage)?;
        temporary.as_file().sync_all().map_err(storage)?;
        temporary
            .persist(self.directory.join("labels.json"))
            .map_err(storage)?;
        Ok(saved.labels)
    }
}
