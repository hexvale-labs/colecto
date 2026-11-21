//! Pure conversation threading and subject normalization helpers.
use crate::Message;
use std::collections::{BTreeMap, HashSet};

/// A message paired with the folder it came from in a conversation thread.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ThreadMessage {
    pub folder: String,
    pub message: Message,
}

/// A grouped email conversation thread.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ConversationThread {
    pub id: String,
    pub subject: String,
    pub messages: Vec<Message>,
    pub unread_count: usize,
    pub flagged: bool,
    pub latest_date: String,
    pub participants: Vec<String>,
}

/// Strip leading `Re:`, `Fwd:`, and `Fw:` prefixes (case-insensitive) for display grouping.
pub fn normalize_subject(subject: &str) -> &str {
    let mut rest = subject.trim();
    loop {
        let lower = rest.to_ascii_lowercase();
        if let Some(after) = lower
            .strip_prefix("re:")
            .or_else(|| lower.strip_prefix("fwd:"))
            .or_else(|| lower.strip_prefix("fw:"))
        {
            let consumed = lower.len() - after.len();
            rest = rest[consumed..].trim_start();
        } else {
            break;
        }
    }
    rest
}

/// Group flat folder messages into conversations.
///
/// Messages with an empty `thread` field stay single-message threads keyed by `message.id`.
/// Input order is assumed newest-first as returned by IMAP/JMAP queries; within each thread
/// messages are ordered chronologically (oldest first), and threads themselves are ordered
/// latest-first by their newest message.
pub fn group_conversations(messages: &[Message]) -> Vec<ConversationThread> {
    let mut order: Vec<String> = Vec::new();
    let mut grouped: BTreeMap<String, Vec<Message>> = BTreeMap::new();

    for message in messages {
        let key = if message.thread.is_empty() {
            format!("msg:{}", message.id)
        } else {
            format!("thread:{}", message.thread)
        };
        if !grouped.contains_key(&key) {
            order.push(key.clone());
        }
        grouped.entry(key).or_default().push(message.clone());
    }

    order
        .into_iter()
        .filter_map(|key| {
            let mut items = grouped.remove(&key)?;
            let latest = items.first()?.clone();
            // Reverse newest-first folder slice so conversation reads chronologically oldest -> newest.
            items.reverse();
            let unread_count = items.iter().filter(|m| !m.seen).count();
            let flagged = items.iter().any(|m| m.flagged);
            let mut seen_participants = HashSet::new();
            let mut participants = Vec::new();
            for m in &items {
                let name = if m.sender.trim().is_empty() {
                    m.address.trim()
                } else {
                    m.sender.trim()
                };
                if !name.is_empty() && seen_participants.insert(name.to_ascii_lowercase()) {
                    participants.push(name.to_owned());
                }
            }
            let subject = normalize_subject(&items[0].subject);
            let subject = if subject.is_empty() {
                latest.subject.clone()
            } else {
                subject.to_owned()
            };
            let id = if latest.thread.is_empty() {
                latest.id.clone()
            } else {
                latest.thread.clone()
            };
            Some(ConversationThread {
                id,
                subject,
                messages: items,
                unread_count,
                flagged,
                latest_date: latest.date,
                participants,
            })
        })
        .collect()
}

/// Collect related messages for `current` across `current_folder` and `other_folders`,
/// deduplicated by message ID, sorted chronologically, and windowed around `current` up to
/// `max_messages` (defaulting to 20 when `0` is passed).
pub fn conversation_for(
    current_folder: &str,
    current: &Message,
    other_folders: &[(&str, &[Message])],
    max_messages: usize,
) -> Vec<ThreadMessage> {
    if current.thread.is_empty() {
        return Vec::new();
    }
    let limit = if max_messages == 0 { 20 } else { max_messages };
    let mut seen = HashSet::new();
    let mut thread = Vec::new();
    seen.insert(current.id.clone());
    thread.push(ThreadMessage {
        folder: current_folder.to_owned(),
        message: current.clone(),
    });

    for (folder, messages) in other_folders {
        for message in *messages {
            if message.thread == current.thread && seen.insert(message.id.clone()) {
                thread.push(ThreadMessage {
                    folder: (*folder).to_owned(),
                    message: message.clone(),
                });
            }
        }
    }

    if thread.len() <= 1 {
        return Vec::new();
    }
    thread.sort_by(|a, b| a.message.date.cmp(&b.message.date));
    let at = thread
        .iter()
        .position(|item| item.message.id == current.id)
        .unwrap_or(0);
    let half = limit / 2;
    let start = at
        .saturating_sub(half)
        .min(thread.len().saturating_sub(limit));
    let end = (start + limit).min(thread.len());
    thread[start..end].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(id: &str, sender: &str, subject: &str, date: &str, thread: &str, seen: bool) -> Message {
        Message {
            seen,
            flagged: false,
            id: id.into(),
            sender: sender.into(),
            address: format!("{sender}@example.test").to_ascii_lowercase(),
            subject: subject.into(),
            body: format!("body {id}"),
            date: date.into(),
            thread: thread.into(),
        }
    }

    #[test]
    fn normalizes_repeated_reply_and_forward_prefixes() {
        assert_eq!(
            normalize_subject("Re: Fwd: fw:  Quarterly plan"),
            "Quarterly plan"
        );
        assert_eq!(normalize_subject("RE: Hello"), "Hello");
        assert_eq!(normalize_subject("Plain subject"), "Plain subject");
    }

    #[test]
    fn groups_conversations_and_tracks_unread_and_participants() {
        // Folder messages arrive newest-first.
        let messages = vec![
            msg("3", "Bob", "Re: Launch", "2026-11-03", "t1", false),
            msg("2", "Solo", "Alone", "2026-11-02", "", true),
            msg("1", "Alice", "Launch", "2026-11-01", "t1", true),
        ];
        let threads = group_conversations(&messages);
        assert_eq!(threads.len(), 2);
        assert_eq!(threads[0].id, "t1");
        assert_eq!(threads[0].subject, "Launch");
        assert_eq!(threads[0].messages.len(), 2);
        assert_eq!(threads[0].messages[0].id, "1");
        assert_eq!(threads[0].messages[1].id, "3");
        assert_eq!(threads[0].unread_count, 1);
        assert_eq!(threads[0].participants, vec!["Alice", "Bob"]);
        assert_eq!(threads[1].id, "2");
        assert_eq!(threads[1].messages.len(), 1);
    }

    #[test]
    fn conversation_for_deduplicates_across_folders_and_windows() {
        let current = msg("m2", "Bob", "Re: Topic", "2026-11-02", "t1", true);
        let inbox = vec![
            current.clone(),
            msg("m1", "Alice", "Topic", "2026-11-01", "t1", true),
            msg("other", "Dan", "Unrelated", "2026-11-04", "t2", false),
        ];
        let sent = vec![
            current.clone(),
            msg("m3", "Me", "Re: Topic", "2026-11-03", "t1", true),
        ];
        let conv = conversation_for("inbox", &current, &[("inbox", &inbox), ("sent", &sent)], 20);
        assert_eq!(conv.len(), 3);
        assert_eq!(conv[0].message.id, "m1");
        assert_eq!(conv[1].message.id, "m2");
        assert_eq!(conv[1].folder, "inbox");
        assert_eq!(conv[2].message.id, "m3");
        assert_eq!(conv[2].folder, "sent");

        // Single-message or empty-thread cases return empty vec.
        let single = msg("s1", "A", "S", "2026-11-01", "", true);
        assert!(conversation_for("inbox", &single, &[("inbox", &inbox)], 20).is_empty());
    }
}
