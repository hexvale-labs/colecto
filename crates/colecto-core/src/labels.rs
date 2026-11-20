use super::*;

pub type Labels = std::collections::BTreeMap<String, Vec<String>>;

const MAX_LABELS: usize = 50;
const MAX_LABEL_MESSAGES: usize = 1000;

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name == name.trim()
        && name.len() <= 80
        && !name.chars().any(char::is_control)
}
fn valid_message(id: &str) -> bool {
    !id.is_empty() && id.len() <= 1024 && !id.chars().any(char::is_control)
}
impl MailStore {
    pub fn labels(&self) -> Result<Labels, Error> {
        let account = self.id.as_str();
        let mut labels = Labels::new();
        for row in self.database.query(
            "SELECT name FROM labels WHERE account=?1",
            vec![account.into()],
        )? {
            labels.insert(row.text(0)?, Vec::new());
        }
        for row in self.database.query(
            "SELECT label,message FROM label_messages WHERE account=?1 ORDER BY rowid",
            vec![account.into()],
        )? {
            let (label, message) = (row.text(0)?, row.text(1)?);
            labels
                .get_mut(&label)
                .ok_or_else(|| storage("Labelled message without a label"))?
                .push(message);
        }
        if labels.len() > MAX_LABELS
            || labels.iter().any(|(name, ids)| {
                !valid_name(name)
                    || ids.len() > MAX_LABEL_MESSAGES
                    || !ids.iter().all(|id| valid_message(id))
            })
        {
            return Err(Error::Storage("Invalid label store".into()));
        }
        Ok(labels)
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
        let labels = self.labels()?;
        let account = self.id.as_str();
        let statements: Vec<(&str, Vec<Value>)> = match (message_id, remove) {
            (Some(id), _) if !valid_message(id) || self.message(id).is_none() => {
                return Err(Error::Missing);
            }
            (Some(id), remove) => {
                let ids = labels.get(name).ok_or(Error::Missing)?;
                let present = ids.iter().any(|entry| entry == id);
                if !remove && !present && ids.len() >= MAX_LABEL_MESSAGES {
                    return Err(Error::Invalid(format!(
                        "A label can contain at most {MAX_LABEL_MESSAGES} messages"
                    )));
                }
                let sql = if remove {
                    "DELETE FROM label_messages WHERE account=?1 AND label=?2 AND message=?3"
                } else {
                    "INSERT OR IGNORE INTO label_messages(account,label,message) VALUES (?1,?2,?3)"
                };
                vec![(sql, vec![account.into(), name.into(), id.into()])]
            }
            (None, true) => vec![
                (
                    "DELETE FROM label_messages WHERE account=?1 AND label=?2",
                    vec![account.into(), name.into()],
                ),
                (
                    "DELETE FROM labels WHERE account=?1 AND name=?2",
                    vec![account.into(), name.into()],
                ),
            ],
            (None, false) => {
                if !labels.contains_key(name) && labels.len() >= MAX_LABELS {
                    return Err(Error::Invalid(format!(
                        "An account can have at most {MAX_LABELS} labels"
                    )));
                }
                vec![(
                    "INSERT OR IGNORE INTO labels(account,name) VALUES (?1,?2)",
                    vec![account.into(), name.into()],
                )]
            }
        };
        self.database.write(statements)?;
        self.labels()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn labels_are_bounded_and_failed_updates_change_nothing() {
        let directory = tempfile::tempdir().unwrap();
        let mut store = MailStore::open(directory.path()).unwrap();
        for index in 0..50 {
            store
                .update_label(&format!("Label {index}"), None, false)
                .unwrap();
        }
        let before = store.labels().unwrap();
        assert!(store.update_label("Extra", None, false).is_err());
        assert!(store.update_label(&"界".repeat(27), None, false).is_err());
        // Adding an existing label again is not a new one.
        store.update_label("Label 3", None, false).unwrap();
        assert_eq!(store.labels().unwrap(), before);
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
