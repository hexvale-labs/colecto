//! Durable, immutable automation delivery requests.
use crate::api_keys::random;
use colecto_core::{Database, DraftInput, Row, Value, validate_draft};
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
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

/// The journal lives in `send_approvals`; this is its copy in memory, which every change
/// updates first so a request that began submitting is never offered again, even if the
/// write fails.
pub struct Approvals {
    database: Database,
    requests: Vec<PendingSend>,
}

impl Status {
    fn as_str(self) -> &'static str {
        match self {
            Status::Pending => "pending",
            Status::Rejected => "rejected",
            Status::Expired => "expired",
            Status::Submitting => "submitting",
            Status::Sent => "sent",
            Status::Uncertain => "uncertain",
        }
    }
}

fn invalid<E>(_: E) -> String {
    "Invalid approval journal".into()
}

fn read_request(row: &Row) -> Result<PendingSend, String> {
    let time = |index| {
        row.integer(index)
            .map_err(invalid)
            .and_then(|t| u64::try_from(t).map_err(invalid))
    };
    Ok(PendingSend {
        id: row.text(0).map_err(invalid)?,
        requester: row.text(1).map_err(invalid)?,
        account: row.text(2).map_err(invalid)?,
        from: row.text(3).map_err(invalid)?,
        snapshot: row
            .optional_text(4)
            .map_err(invalid)?
            .map(|json| serde_json::from_str(&json).map_err(invalid))
            .transpose()?,
        created_at: time(5)?,
        expires_at: time(6)?,
        status: serde_json::from_value(serde_json::Value::String(row.text(7).map_err(invalid)?))
            .map_err(invalid)?,
    })
}

fn time(value: u64) -> Value {
    i64::try_from(value).unwrap_or(i64::MAX).into()
}

const PERSIST_FAILED: &str = "Could not persist delivery request; do not retry delivery";

impl Approvals {
    pub fn open(database: Database, now: u64) -> Result<Self, String> {
        // Persist crash recovery before accepting new decisions.
        database
            .write(vec![(
                "UPDATE send_approvals SET status='uncertain' WHERE status='submitting'",
                vec![],
            )])
            .map_err(|_| PERSIST_FAILED)?;
        let requests = database
            .query(
                "SELECT id,requester,account,sender,snapshot,created_at,expires_at,status FROM send_approvals ORDER BY rowid",
                vec![],
            )
            .map_err(|_| "Invalid approval journal")?
            .iter()
            .map(read_request)
            .collect::<Result<Vec<_>, _>>()?;
        if requests.len() > MAX_REQUESTS {
            return Err("Invalid approval journal".into());
        }
        let mut result = Self { database, requests };
        result.maintain(now)?;
        Ok(result)
    }

    fn persist(&self, statements: Vec<(&str, Vec<Value>)>) -> Result<(), String> {
        self.database
            .write(statements)
            .map_err(|_| PERSIST_FAILED.into())
    }

    pub(crate) fn maintain(&mut self, now: u64) -> Result<(), String> {
        if !self.requests.iter().any(|r| {
            now >= r.created_at.saturating_add(RETENTION)
                || (r.status == Status::Pending && now >= r.expires_at)
        }) {
            return Ok(());
        }
        self.requests
            .retain(|r| now < r.created_at.saturating_add(RETENTION));
        for r in &mut self.requests {
            if r.status == Status::Pending && now >= r.expires_at {
                r.status = Status::Expired;
            }
        }
        self.persist(vec![
            (
                "DELETE FROM send_approvals WHERE created_at + ?1 <= ?2",
                vec![time(RETENTION), time(now)],
            ),
            (
                "UPDATE send_approvals SET status='expired' WHERE status='pending' AND expires_at <= ?1",
                vec![time(now)],
            ),
        ])
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
        if self.requests.len() >= MAX_REQUESTS
            || self
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
        validate_draft(&snapshot).map_err(|_| "Invalid outgoing message")?;
        // Approvals show the owner text only, so they never carry files or forwards.
        if !snapshot.attachments.is_empty() || snapshot.forward_of.is_some() {
            return Err("Delivery requests cannot include attachments or forwards".into());
        }
        let json = serde_json::to_string(&snapshot).map_err(|_| "Invalid outgoing message")?;
        if json.len() > 110_000 {
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
        self.persist(vec![(
            "INSERT INTO send_approvals(id,requester,account,sender,snapshot,created_at,expires_at,status) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            vec![
                request.id.as_str().into(),
                requester.into(),
                account.into(),
                from.into(),
                json.into(),
                time(request.created_at),
                time(request.expires_at),
                request.status.as_str().into(),
            ],
        )])?;
        self.requests.push(request.clone());
        Ok(request)
    }

    pub fn list(&mut self, now: u64) -> Result<Vec<PendingSend>, String> {
        self.maintain(now)?;
        Ok(self.requests.clone())
    }

    pub fn get(&mut self, id: &str, now: u64) -> Result<PendingSend, String> {
        self.maintain(now)?;
        self.requests
            .iter()
            .find(|r| r.id == id)
            .cloned()
            .ok_or_else(|| "Delivery request not found".into())
    }

    /// Move a request from `from` to `to`, in memory first so the advanced state holds
    /// even if the write fails.
    fn advance(&mut self, id: &str, from: Status, to: Status) -> Result<PendingSend, String> {
        let request = self
            .requests
            .iter_mut()
            .find(|r| r.id == id)
            .ok_or("Delivery request not found")?;
        if request.status != from {
            return Err(match from {
                Status::Submitting => "Delivery request is not submitting",
                _ => "Delivery request is no longer pending",
            }
            .into());
        }
        request.status = to;
        let request = request.clone();
        self.persist(vec![(
            "UPDATE send_approvals SET status=?2 WHERE id=?1",
            vec![id.into(), to.as_str().into()],
        )])?;
        Ok(request)
    }

    pub fn reject(&mut self, id: &str, now: u64) -> Result<PendingSend, String> {
        self.get(id, now)?;
        self.advance(id, Status::Pending, Status::Rejected)
    }

    pub fn begin(&mut self, id: &str, now: u64) -> Result<DraftInput, String> {
        let request = self.get(id, now)?;
        let snapshot = request.snapshot.ok_or("Snapshot unavailable")?;
        self.advance(id, Status::Pending, Status::Submitting)?;
        Ok(snapshot)
    }

    pub fn finish(&mut self, id: &str, sent: bool) -> Result<(), String> {
        let to = if sent {
            Status::Sent
        } else {
            Status::Uncertain
        };
        self.advance(id, Status::Submitting, to).map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let database = Database::open(dir.path()).unwrap();
        let mut approvals = Approvals::open(database.clone(), 10).unwrap();
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
        let mut approvals = Approvals::open(database.clone(), 12).unwrap();
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
        let database = Database::open(dir.path()).unwrap();
        let mut approvals = Approvals::open(database.clone(), 10).unwrap();
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
        let database = Database::open(dir.path()).unwrap();
        let mut approvals = Approvals::open(database.clone(), 10).unwrap();
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
        let mut approvals = Approvals::open(database.clone(), 12).unwrap();
        assert_eq!(approvals.get(&c.id, 12).unwrap().status, Status::Sent);
        assert!(approvals.begin(&c.id, 12).is_err());
    }
}
