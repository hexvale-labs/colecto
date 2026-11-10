//! Durable, immutable automation delivery requests.
use crate::api_keys::random;
use colecto_extension_api::DraftInput;
use serde::{Deserialize, Serialize};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

const RETENTION: u64 = 24 * 60 * 60;
const MAX_REQUESTS: usize = 128;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Pending,
    Rejected,
    Expired,
    Submitting,
    Sent,
    Uncertain,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingSend {
    pub id: String,
    pub requester: String,
    pub account: String,
    pub from: String,
    pub snapshot: Option<DraftInput>,
    pub created_at: u64,
    pub expires_at: u64,
    pub status: Status,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal {
    version: u32,
    requests: Vec<PendingSend>,
}
pub struct Approvals {
    path: PathBuf,
    journal: Journal,
}
impl Approvals {
    pub fn open(directory: &Path, now: u64) -> Result<Self, String> {
        let path = directory.join("send-approvals.json");
        let mut journal: Journal = match std::fs::symlink_metadata(&path) {
            Ok(meta) => {
                if !meta.is_file() || meta.len() > 16_000_000 {
                    return Err("Invalid approval journal".into());
                }
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if meta.permissions().mode() & 0o077 != 0 {
                        return Err("Approval journal must be private".into());
                    }
                }
                serde_json::from_slice(
                    &std::fs::read(&path).map_err(|_| "Cannot read approval journal")?,
                )
                .map_err(|_| "Invalid approval journal")?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Journal {
                version: 1,
                requests: vec![],
            },
            Err(_) => return Err("Cannot read approval journal".into()),
        };
        if journal.version != 1 || journal.requests.len() > MAX_REQUESTS {
            return Err("Invalid approval journal".into());
        }
        for request in &mut journal.requests {
            if request.status == Status::Submitting {
                request.status = Status::Uncertain;
            }
        }
        let mut result = Self { path, journal };
        result.maintain(now)?;
        // Persist crash recovery before accepting new decisions.
        result.persist(&result.journal)?;
        Ok(result)
    }
    fn persist(&self, journal: &Journal) -> Result<(), String> {
        let temporary = self.path.with_extension(format!("{}.tmp", random()?));
        let result = (|| -> Result<(), Box<dyn std::error::Error>> {
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temporary)?;
            file.write_all(&serde_json::to_vec(journal)?)?;
            file.sync_all()?;
            std::fs::rename(&temporary, &self.path)?;
            #[cfg(unix)]
            std::fs::File::open(self.path.parent().unwrap())?.sync_all()?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(temporary);
        }
        result.map_err(|_| "Could not persist delivery request; do not retry delivery".into())
    }
    fn commit(&mut self, journal: Journal) -> Result<(), String> {
        // Keep the advanced state even if a directory sync fails after rename.
        self.journal = journal;
        self.persist(&self.journal)
    }
    pub(crate) fn maintain(&mut self, now: u64) -> Result<(), String> {
        if !self.journal.requests.iter().any(|r| {
            now >= r.created_at.saturating_add(RETENTION)
                || (r.status == Status::Pending && now >= r.expires_at)
        }) {
            return Ok(());
        }
        let mut journal = self.journal.clone();
        journal
            .requests
            .retain(|r| now < r.created_at.saturating_add(RETENTION));
        for r in &mut journal.requests {
            if r.status == Status::Pending && now >= r.expires_at {
                r.status = Status::Expired;
            }
        }
        self.commit(journal)
    }
    pub fn create(
        &mut self,
        requester: &str,
        account: &str,
        from: &str,
        snapshot: DraftInput,
        now: u64,
    ) -> Result<PendingSend, String> {
        self.maintain(now)?;
        if self.journal.requests.len() >= MAX_REQUESTS
            || self
                .journal
                .requests
                .iter()
                .filter(|r| r.requester == requester && r.status == Status::Pending)
                .count()
                >= 16
        {
            return Err(
                "Delivery request limit reached; wait for existing requests to expire".into(),
            );
        }
        colecto_core::validate_draft(&snapshot).map_err(|_| "Invalid outgoing message")?;
        // Approvals show the owner text only, so they never carry files or forwards.
        if !snapshot.attachments.is_empty() || snapshot.forward_of.is_some() {
            return Err("Delivery requests cannot include attachments or forwards".into());
        }
        if serde_json::to_vec(&snapshot)
            .map_err(|_| "Invalid outgoing message")?
            .len()
            > 110_000
        {
            return Err("Outgoing message exceeds approval limit".into());
        }
        let request = PendingSend {
            id: random()?,
            requester: requester.into(),
            account: account.into(),
            from: from.into(),
            snapshot: Some(snapshot),
            created_at: now,
            expires_at: now.saturating_add(600),
            status: Status::Pending,
        };
        let mut journal = self.journal.clone();
        journal.requests.push(request.clone());
        self.commit(journal)?;
        Ok(request)
    }
    pub fn list(&mut self, now: u64) -> Result<Vec<PendingSend>, String> {
        self.maintain(now)?;
        Ok(self.journal.requests.clone())
    }
    pub fn get(&mut self, id: &str, now: u64) -> Result<PendingSend, String> {
        self.maintain(now)?;
        self.journal
            .requests
            .iter()
            .find(|r| r.id == id)
            .cloned()
            .ok_or_else(|| "Delivery request not found".into())
    }
    pub fn reject(&mut self, id: &str, now: u64) -> Result<PendingSend, String> {
        self.get(id, now)?;
        let mut journal = self.journal.clone();
        let request = journal
            .requests
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or("Delivery request not found")?;
        if request.status != Status::Pending {
            return Err("Delivery request is no longer pending".into());
        }
        request.status = Status::Rejected;
        let result = request.clone();
        self.commit(journal)?;
        Ok(result)
    }
    pub fn begin(&mut self, id: &str, now: u64) -> Result<DraftInput, String> {
        self.get(id, now)?;
        let mut journal = self.journal.clone();
        let request = journal
            .requests
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or("Delivery request not found")?;
        if request.status != Status::Pending {
            return Err("Delivery request is no longer pending".into());
        }
        let snapshot = request.snapshot.clone().ok_or("Snapshot unavailable")?;
        request.status = Status::Submitting;
        self.commit(journal)?;
        Ok(snapshot)
    }
    pub fn finish(&mut self, id: &str, sent: bool) -> Result<(), String> {
        let mut journal = self.journal.clone();
        let request = journal
            .requests
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or("Delivery request not found")?;
        if request.status != Status::Submitting {
            return Err("Delivery request is not submitting".into());
        }
        request.status = if sent {
            Status::Sent
        } else {
            Status::Uncertain
        };
        self.commit(journal)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use colecto_extension_api::DraftInput;
    fn input() -> DraftInput {
        DraftInput {
            id: None,
            recipient: "a@example.test".into(),
            subject: "reviewed".into(),
            body: "original".into(),
            ..Default::default()
        }
    }
    #[test]
    fn snapshot_replay_expiry_and_crash() {
        let dir = tempfile::tempdir().unwrap();
        let mut approvals = Approvals::open(dir.path(), 10).unwrap();
        let mut draft = input();
        let request = approvals
            .create("key", "account", "from@example.test", draft.clone(), 10)
            .unwrap();
        draft.body = "edited".into();
        assert_eq!(
            approvals
                .get(&request.id, 11)
                .unwrap()
                .snapshot
                .unwrap()
                .body,
            "original"
        );
        let snapshot = approvals.begin(&request.id, 11).unwrap();
        assert_eq!(snapshot.body, "original");
        assert!(approvals.begin(&request.id, 11).is_err());
        drop(approvals);
        let mut approvals = Approvals::open(dir.path(), 12).unwrap();
        assert_eq!(
            approvals.get(&request.id, 12).unwrap().status,
            Status::Uncertain
        );
        assert!(approvals.begin(&request.id, 12).is_err());
        let expired = approvals
            .create("key", "account", "from", input(), 20)
            .unwrap();
        assert!(approvals.begin(&expired.id, 620).is_err());
        assert_eq!(
            approvals.get(&expired.id, 620).unwrap().status,
            Status::Expired
        );
        assert!(approvals.get(&expired.id, 20 + RETENTION).is_err());
    }
    #[test]
    fn bounds_and_validation_prevent_unreviewable_requests() {
        let dir = tempfile::tempdir().unwrap();
        let mut approvals = Approvals::open(dir.path(), 10).unwrap();
        let mut invalid = input();
        invalid.recipient = "not-an-address".into();
        let mut forward = input();
        forward.forward_of = Some("imap-1-1".into());
        assert!(
            approvals
                .create("key", "work", "me@example.test", forward, 10)
                .is_err()
        );
        assert!(
            approvals
                .create("key", "account", "from", invalid, 10)
                .is_err()
        );
        let mut oversized = input();
        oversized.body = "x".repeat(100_001);
        assert!(
            approvals
                .create("key", "account", "from", oversized, 10)
                .is_err()
        );
        for _ in 0..16 {
            approvals
                .create("key", "account", "from", input(), 10)
                .unwrap();
        }
        assert!(
            approvals
                .create("key", "account", "from", input(), 10)
                .is_err()
        );
    }
    #[test]
    fn rejected_and_failed_submission_never_replay() {
        let dir = tempfile::tempdir().unwrap();
        let mut approvals = Approvals::open(dir.path(), 10).unwrap();
        let a = approvals
            .create("key", "account", "from", input(), 10)
            .unwrap();
        approvals.reject(&a.id, 11).unwrap();
        assert!(approvals.begin(&a.id, 11).is_err());
        let b = approvals
            .create("key", "account", "from", input(), 10)
            .unwrap();
        approvals.begin(&b.id, 11).unwrap();
        approvals.finish(&b.id, false).unwrap();
        assert_eq!(approvals.get(&b.id, 11).unwrap().status, Status::Uncertain);
        assert!(approvals.begin(&b.id, 12).is_err());
        let c = approvals
            .create("key", "account", "from", input(), 10)
            .unwrap();
        approvals.begin(&c.id, 11).unwrap();
        approvals.finish(&c.id, true).unwrap();
        drop(approvals);
        let mut approvals = Approvals::open(dir.path(), 12).unwrap();
        assert_eq!(approvals.get(&c.id, 12).unwrap().status, Status::Sent);
        assert!(approvals.begin(&c.id, 12).is_err());
    }
}
