//! Search filters typed into a search: `from:`, `to:`, `is:starred`, `is:unread`,
//! `is:read`, `after:YYYY-MM-DD` and `before:YYYY-MM-DD`, combined with plain words. Each
//! becomes the provider's own criterion (IMAP SEARCH keys, JMAP filter conditions), so search
//! still runs on the server. Anything else, including `word:` prefixes we do not know, stays
//! plain text.

/// A calendar day as year, month and day, checked to be a real date.
pub type Day = (u16, u8, u8);

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Query {
    pub text: String,
    pub from: Vec<String>,
    pub to: Vec<String>,
    pub starred: Option<bool>,
    pub unread: Option<bool>,
    /// On or after this day.
    pub after: Option<Day>,
    /// Before this day.
    pub before: Option<Day>,
}

impl Query {
    pub fn parse(search: &str) -> Self {
        let mut query = Query::default();
        let mut words = Vec::new();
        for token in tokens(search) {
            let known = token.split_once(':').and_then(|(key, value)| {
                let value = value.trim_matches('"');
                match key.to_ascii_lowercase().as_str() {
                    "from" if !value.is_empty() => query.from.push(value.to_owned()),
                    "to" if !value.is_empty() => query.to.push(value.to_owned()),
                    "is" => match value.to_ascii_lowercase().as_str() {
                        "starred" | "flagged" => query.starred = Some(true),
                        "unstarred" => query.starred = Some(false),
                        "unread" => query.unread = Some(true),
                        "read" => query.unread = Some(false),
                        _ => return None,
                    },
                    "after" => query.after = Some(day(value)?),
                    "before" => query.before = Some(day(value)?),
                    _ => return None,
                }
                Some(())
            });
            if known.is_none() {
                words.push(token);
            }
        }
        query.text = words.join(" ");
        query
    }

    /// The same filters over saved messages, for the demo account, which has no server.
    /// Its messages carry no recipients or real dates: `to:` matches nothing, and `after:`
    /// and `before:` are ignored.
    pub fn matches(&self, message: &crate::Message) -> bool {
        let text = self.text.to_lowercase();
        let contains =
            |field: &str, needle: &str| field.to_lowercase().contains(&needle.to_lowercase());
        [
            &message.sender,
            &message.address,
            &message.subject,
            &message.body,
        ]
        .iter()
        .any(|field| field.to_lowercase().contains(&text))
            && self
                .from
                .iter()
                .all(|from| contains(&message.sender, from) || contains(&message.address, from))
            && self.to.is_empty()
            && self
                .starred
                .is_none_or(|starred| message.flagged == starred)
            && self.unread.is_none_or(|unread| message.seen != unread)
    }

    /// IMAP `UID SEARCH` criteria (RFC 3501 §6.4.4); `ALL` when nothing narrows it.
    pub fn imap(&self) -> String {
        let quote =
            |value: &str| format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""));
        let mut keys = Vec::new();
        if !self.text.is_empty() {
            keys.push(format!("TEXT {}", quote(&self.text)));
        }
        keys.extend(self.from.iter().map(|from| format!("FROM {}", quote(from))));
        keys.extend(self.to.iter().map(|to| format!("TO {}", quote(to))));
        match self.starred {
            Some(true) => keys.push("FLAGGED".into()),
            Some(false) => keys.push("UNFLAGGED".into()),
            None => {}
        }
        match self.unread {
            Some(true) => keys.push("UNSEEN".into()),
            Some(false) => keys.push("SEEN".into()),
            None => {}
        }
        if let Some(day) = self.after {
            keys.push(format!("SINCE {}", imap_date(day)));
        }
        if let Some(day) = self.before {
            keys.push(format!("BEFORE {}", imap_date(day)));
        }
        if keys.is_empty() {
            keys.push("ALL".into());
        }
        let strings = [&self.text].into_iter().chain(&self.from).chain(&self.to);
        let charset = if strings.into_iter().all(|s| s.is_ascii()) {
            ""
        } else {
            "CHARSET UTF-8 "
        };
        format!("{charset}{}", keys.join(" "))
    }

    /// JMAP `Email/query` filter conditions (RFC 8621 §4.4.1), to be combined with `AND`.
    pub fn jmap(&self) -> Vec<serde_json::Value> {
        use serde_json::json;
        let mut conditions = Vec::new();
        if !self.text.is_empty() {
            conditions.push(json!({ "text": self.text }));
        }
        conditions.extend(self.from.iter().map(|from| json!({ "from": from })));
        conditions.extend(self.to.iter().map(|to| json!({ "to": to })));
        match self.starred {
            Some(true) => conditions.push(json!({ "hasKeyword": "$flagged" })),
            Some(false) => conditions.push(json!({ "notKeyword": "$flagged" })),
            None => {}
        }
        match self.unread {
            Some(true) => conditions.push(json!({ "notKeyword": "$seen" })),
            Some(false) => conditions.push(json!({ "hasKeyword": "$seen" })),
            None => {}
        }
        // `after` is inclusive and `before` exclusive in both protocols, as here.
        if let Some((y, m, d)) = self.after {
            conditions.push(json!({ "after": format!("{y:04}-{m:02}-{d:02}T00:00:00Z") }));
        }
        if let Some((y, m, d)) = self.before {
            conditions.push(json!({ "before": format!("{y:04}-{m:02}-{d:02}T00:00:00Z") }));
        }
        conditions
    }
}

/// Words, keeping `key:"two words"` together.
fn tokens(search: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    for c in search.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                current.push(c);
            }
            c if c.is_whitespace() && !quoted => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            c => current.push(c),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

fn day(value: &str) -> Option<Day> {
    let mut parts = value.split('-');
    let year: u16 = parts.next()?.parse().ok()?;
    let month: u8 = parts.next()?.parse().ok()?;
    let day: u8 = parts.next()?.parse().ok()?;
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    (parts.next().is_none()
        && (1970..=9999).contains(&year)
        && (1..=12).contains(&month)
        && day >= 1
        && day <= days[month as usize - 1])
        .then_some((year, month, day))
}

fn imap_date((year, month, day): Day) -> String {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!("{day}-{}-{year}", MONTHS[month as usize - 1])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_become_provider_criteria_and_the_rest_stays_text() {
        let query = Query::parse(
            "invoice from:billing@shop.test to:\"Ada L\" is:starred is:unread after:2026-01-31 before:2026-03-01 https://x.test note:1",
        );
        assert_eq!(query.text, "invoice https://x.test note:1");
        assert_eq!(query.from, ["billing@shop.test"]);
        assert_eq!(query.to, ["Ada L"]);
        assert_eq!(
            query.imap(),
            r#"TEXT "invoice https://x.test note:1" FROM "billing@shop.test" TO "Ada L" FLAGGED UNSEEN SINCE 31-Jan-2026 BEFORE 1-Mar-2026"#
        );
        let jmap = serde_json::Value::Array(query.jmap()).to_string();
        assert!(jmap.contains(r#"{"hasKeyword":"$flagged"}"#));
        assert!(jmap.contains(r#"{"notKeyword":"$seen"}"#));
        assert!(jmap.contains(r#"{"after":"2026-01-31T00:00:00Z"}"#));
        assert!(jmap.contains(r#"{"to":"Ada L"}"#));
    }

    #[test]
    fn bad_dates_and_unknown_values_stay_plain_text_and_strings_are_escaped() {
        let query = Query::parse("after:2026-02-30 is:bogus from:");
        assert_eq!(query.text, "after:2026-02-30 is:bogus from:");
        assert_eq!(query.after, None);
        assert_eq!(Query::parse("is:read").imap(), "SEEN");
        assert_eq!(Query::parse("").imap(), "ALL");
        assert_eq!(
            Query::parse(r#"say "hi\" "#).imap(),
            r#"TEXT "say \"hi\\\"""#
        );
        assert_eq!(
            Query::parse("世界 from:ada").imap(),
            r#"CHARSET UTF-8 TEXT "世界" FROM "ada""#
        );
        assert_eq!(Query::parse("after:2024-02-29").after, Some((2024, 2, 29)));
    }
}
