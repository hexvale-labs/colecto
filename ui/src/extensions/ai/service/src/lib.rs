//! The AI extension's service part. The Host runs it in its Wasm sandbox whenever new mail
//! arrives, with or without a window open: it applies the owner's label rules itself and asks
//! the decision model (through the Host, which holds the provider and key) about the rest.
#![forbid(unsafe_code)]
use colecto_extension::prelude::*;
use colecto_extension::serde_json::json;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Decision model questions per new-mail pass; the Host also caps them per hour.
const DECISIONS_PER_PASS: usize = 10;
/// Messages remembered per account, so each is only ever checked once.
const SEEN_PER_ACCOUNT: usize = 500;
const DECISION_EMAIL_CHARS: usize = 8_000;
const MAX_PATTERN_BYTES: usize = 300;

#[derive(Clone, Copy, Deserialize, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
enum Field {
    Sender,
    Subject,
    Body,
    Any,
}
#[derive(Clone, Deserialize, Debug)]
struct Rule {
    field: Field,
    pattern: String,
}
#[derive(Clone, Default, Deserialize, Debug)]
#[serde(default)]
struct LabelRule {
    enabled: bool,
    model: bool,
    description: String,
    rules: Vec<Rule>,
}
#[derive(Default, Deserialize)]
#[serde(default)]
struct Decision {
    threshold: f64,
}
#[derive(Default, Deserialize)]
#[serde(default)]
struct Settings {
    enabled: bool,
    autolabel: bool,
    decision: Decision,
    labels: BTreeMap<String, BTreeMap<String, LabelRule>>,
}

#[derive(Serialize, Deserialize)]
struct Labeled {
    account: String,
}
colecto_extension::bus_event!(Labeled, bus = "ai", version = 1, name = "labeled");

/// The same rules as the Host's preview in Settings → AI, so a rule matches what it showed.
pub fn compile(pattern: &str) -> Option<regex::Regex> {
    if pattern.trim().is_empty() || pattern.len() > MAX_PATTERN_BYTES {
        return None;
    }
    regex::RegexBuilder::new(pattern)
        .case_insensitive(true)
        .size_limit(1 << 20)
        .dfa_size_limit(1 << 20)
        .build()
        .ok()
}

fn matches_any(rules: &[(Field, regex::Regex)], message: &colecto_extension::Message) -> bool {
    rules.iter().any(|(field, re)| {
        let sender = || re.is_match(&message.sender) || re.is_match(&message.address);
        match field {
            Field::Sender => sender(),
            Field::Subject => re.is_match(&message.subject),
            Field::Body => re.is_match(&message.body),
            Field::Any => sender() || re.is_match(&message.subject) || re.is_match(&message.body),
        }
    })
}

fn clip(text: &str, chars: usize) -> &str {
    text.char_indices()
        .nth(chars)
        .map_or(text, |(i, _)| &text[..i])
}

/// Re-decide the labels of these messages now (#95): from a message's menu, or for the newest
/// mail from Settings → AI. Unlike new mail, labels with rules that no longer match are
/// removed, and messages checked before are checked again.
#[derive(Serialize, Deserialize)]
struct Reclassify {
    account: String,
    ids: Vec<String>,
}
#[derive(Serialize, Deserialize, Default, Debug, PartialEq)]
struct Reclassified {
    /// Messages found and checked.
    checked: usize,
    /// Labels added or removed.
    changed: usize,
    /// Messages the decision model was not asked about, because this pass's or the
    /// hour's budget ran out; their model-only labels were left as they were.
    unanswered: usize,
}
colecto_extension::bus_request!(Reclassify => Reclassified, bus = "ai", version = 1, name = "reclassify");

/// At most this many messages per reclassify request; callers batch larger runs.
const RECLASSIFY_BATCH: usize = 25;

fn on_mail(event: MailEvent, cx: &mut Context) -> Result<()> {
    classify(&event.account, &event.ids, false, cx).map(|_| ())
}

fn reclassify(request: Reclassify, cx: &mut Context) -> Result<Reclassified> {
    let ids: Vec<String> = request.ids.into_iter().take(RECLASSIFY_BATCH).collect();
    classify(&request.account, &ids, true, cx)
}

fn classify(account: &str, ids: &[String], force: bool, cx: &mut Context) -> Result<Reclassified> {
    let mut outcome = Reclassified::default();
    let settings: Settings = cx.call(&json!({"operation": "ai.settings"}))?;
    // New mail also needs the automatic-labels switch; a reclassify asked for by hand does not.
    if !settings.enabled || (!force && !settings.autolabel) {
        return Ok(outcome);
    }
    let rules: BTreeMap<String, LabelRule> = settings
        .labels
        .get(account)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter(|(_, r)| r.enabled && (r.model || !r.rules.is_empty()))
        .collect();
    if rules.is_empty() {
        return Ok(outcome);
    }
    // Each new message is checked once, so a label the owner removes stays removed.
    let key = format!(
        "seen.{}",
        account
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .take(40)
            .collect::<String>()
    );
    let mut seen: VecDeque<String> = cx.stored(&key)?.unwrap_or_default();
    let fresh: BTreeSet<String> = ids
        .iter()
        .filter(|id| force || !seen.contains(id))
        .cloned()
        .collect();
    if fresh.is_empty() {
        return Ok(outcome);
    }
    seen.extend(
        fresh
            .iter()
            .filter(|id| !seen.contains(id))
            .cloned()
            .collect::<Vec<_>>(),
    );
    while seen.len() > SEEN_PER_ACCOUNT {
        seen.pop_front();
    }
    cx.store(&key, &seen)?;
    // One message at a time: the whole saved Inbox (`messages.list`, bodies included) does
    // not fit the 64 KiB bridge reply once it holds real mail, which failed every pass.
    let mut messages = Vec::new();
    for id in &fresh {
        if let Ok(message) = cx.message(id) {
            messages.push(message);
        }
    }
    let existing = cx.labels()?;
    let compiled: BTreeMap<&String, Vec<(Field, regex::Regex)>> = rules
        .iter()
        .filter(|(name, _)| existing.contains_key(*name))
        .map(|(name, rule)| {
            (
                name,
                rule.rules
                    .iter()
                    .filter_map(|r| compile(&r.pattern).map(|re| (r.field, re)))
                    .collect(),
            )
        })
        .collect();
    let mut budget = DECISIONS_PER_PASS;
    for message in &messages {
        outcome.checked += 1;
        let has = |name: &String| {
            existing
                .get(name)
                .is_some_and(|ids| ids.contains(&message.id))
        };
        // Labels decided for this message: name → whether it applies.
        let mut decided: BTreeMap<String, bool> = BTreeMap::new();
        let mut ask = Vec::new();
        for (name, res) in &compiled {
            if !force && has(name) {
                continue;
            }
            if matches_any(res, message) {
                decided.insert((*name).clone(), true);
            } else if rules[*name].model {
                ask.push(*name);
            } else {
                decided.insert((*name).clone(), false);
            }
        }
        if !ask.is_empty() && budget == 0 {
            outcome.unanswered += 1;
        }
        if !ask.is_empty() && budget > 0 {
            budget -= 1;
            let questions: Vec<String> = ask
                .iter()
                .map(|name| match rules[*name].description.trim() {
                    "" => format!("This email belongs under the label \"{name}\"."),
                    description => format!("This email belongs under the label \"{name}\". The label is for: {description}"),
                })
                .collect();
            let from = if message.sender.is_empty() || message.sender == message.address {
                message.address.clone()
            } else {
                format!("{} <{}>", message.sender, message.address)
            };
            let state = format!(
                "From: {from}\nSubject: {}\n\n{}",
                message.subject,
                clip(&message.body, DECISION_EMAIL_CHARS)
            );
            match cx.call::<Vec<Option<f64>>>(
                &json!({"operation": "ai.decide", "state": state, "questions": questions}),
            ) {
                Ok(answers) => {
                    for (name, p) in ask.iter().zip(answers) {
                        decided.insert(
                            (*name).clone(),
                            p.is_some_and(|p| p >= settings.decision.threshold),
                        );
                    }
                }
                // The Host reports why; stop asking for this pass.
                Err(_) => {
                    budget = 0;
                    outcome.unanswered += 1;
                }
            }
        }
        for (name, apply) in decided {
            // New mail only ever adds; a reclassify also takes away what no longer applies.
            if apply == has(&name) || (!apply && !force) {
                continue;
            }
            if cx.label(&name, Some(&message.id), !apply).is_ok() {
                outcome.changed += 1;
            }
        }
    }
    if outcome.changed > 0 {
        cx.emit(&Labeled {
            account: account.to_owned(),
        })?;
    }
    Ok(outcome)
}

colecto_extension::export! { mail: on_mail, requests: [reclassify] }

#[cfg(test)]
mod tests {
    use super::*;
    use colecto_extension::serde_json::Value;
    use colecto_extension::testing::TestHost;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn run(settings: Value, decide: Option<f64>) -> (Vec<Value>, Rc<RefCell<Value>>) {
        let log = Rc::new(RefCell::new(Vec::new()));
        let store = Rc::new(RefCell::new(Value::Null));
        let (calls, stored) = (log.clone(), store.clone());
        let mut host = TestHost::new(move |request| {
            calls.borrow_mut().push(request.clone());
            match request["operation"].as_str() {
                Some("ai.settings") => Ok(settings.clone()),
                Some("storage.get") => Ok(stored.borrow().clone()),
                Some("storage.set") => {
                    *stored.borrow_mut() = request["value"].clone();
                    Ok(Value::Null)
                }
                Some("messages.read") => sample(&request["id"]),
                Some("labels.list") => Ok(json!({"Receipts": [], "Friends": []})),
                Some("ai.decide") => Ok(json!([decide])),
                Some("messages.label") => Ok(json!({})),
                Some("bus.emit") => Ok(Value::Null),
                _ => Err("Capability not granted".into()),
            }
        });
        let event = json!({"event": "mail", "account": "a", "ids": ["1", "2"]}).to_string();
        host.run(crate::colecto_main, &event);
        let calls = log.borrow().clone();
        (calls, store)
    }
    fn labeled(calls: &[Value]) -> Vec<(String, String)> {
        calls
            .iter()
            .filter(|c| c["operation"] == "messages.label")
            .map(|c| {
                (
                    c["label"]["name"].as_str().unwrap().into(),
                    c["label"]["message_id"].as_str().unwrap().into(),
                )
            })
            .collect()
    }
    /// Two saved messages, read one at a time as the service part does.
    fn sample(id: &Value) -> Result<Value, String> {
        match id.as_str() {
            Some("1") => Ok(
                json!({"id": "1", "sender": "Shop", "address": "shop@example.test", "subject": "Your receipt", "body": "Thanks"}),
            ),
            Some("2") => Ok(
                json!({"id": "2", "sender": "Ada", "address": "ada@example.test", "subject": "Lunch?", "body": "Friday"}),
            ),
            _ => Err("Message not found".into()),
        }
    }
    fn settings(enabled: bool) -> Value {
        json!({"enabled": enabled, "autolabel": true, "decision": {"threshold": 0.7}, "labels": {"a": {
            "Receipts": {"enabled": true, "model": false, "description": "", "rules": [{"field": "subject", "pattern": "RECEIPT|invoice"}]},
            "Friends": {"enabled": true, "model": true, "description": "people I know", "rules": []},
        }}})
    }

    #[test]
    fn rules_match_case_insensitively_and_the_model_decides_the_rest() {
        let (calls, store) = run(settings(true), Some(0.9));
        assert_eq!(
            labeled(&calls),
            [
                ("Friends".into(), "1".into()),
                ("Receipts".into(), "1".into()),
                ("Friends".into(), "2".into())
            ]
        );
        assert!(calls.iter().any(|c| c["operation"] == "bus.emit"));
        assert_eq!(*store.borrow(), json!(["1", "2"]));
    }

    #[test]
    fn low_confidence_seen_messages_and_ai_off_label_nothing() {
        let (calls, _) = run(settings(true), Some(0.2));
        assert_eq!(labeled(&calls), [("Receipts".into(), "1".into())]);
        let (calls, _) = run(settings(false), Some(0.9));
        assert!(labeled(&calls).is_empty());
        assert!(!calls.iter().any(|c| c["operation"] == "messages.read"));
    }

    #[test]
    fn reclassify_adds_what_matches_now_and_removes_what_no_longer_does() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let calls = log.clone();
        let mut config = settings(true);
        // Asked for by hand, so it runs with automatic labels off too.
        config["autolabel"] = json!(false);
        let mut host = TestHost::new(move |request| {
            calls.borrow_mut().push(request.clone());
            match request["operation"].as_str() {
                Some("ai.settings") => Ok(config.clone()),
                // Both were checked before; a reclassify checks them again.
                Some("storage.get") => Ok(json!(["1", "2"])),
                Some("storage.set") | Some("bus.emit") => Ok(Value::Null),
                Some("messages.read") => sample(&request["id"]),
                Some("labels.list") => Ok(json!({"Receipts": ["2"], "Friends": []})),
                Some("ai.decide") => Ok(json!([0.2])),
                Some("messages.label") => Ok(json!({})),
                _ => Err("Capability not granted".into()),
            }
        });
        let outcome = host
            .with(|cx| {
                reclassify(
                    Reclassify {
                        account: "a".into(),
                        ids: vec!["1".into(), "2".into()],
                    },
                    cx,
                )
            })
            .unwrap();
        let changes: Vec<(String, String, bool)> = log
            .borrow()
            .iter()
            .filter(|c| c["operation"] == "messages.label")
            .map(|c| {
                (
                    c["label"]["name"].as_str().unwrap().into(),
                    c["label"]["message_id"].as_str().unwrap().into(),
                    c["label"]["remove"].as_bool().unwrap(),
                )
            })
            .collect();
        assert_eq!(
            changes,
            [
                ("Receipts".into(), "1".into(), false),
                ("Receipts".into(), "2".into(), true)
            ]
        );
        assert_eq!(
            outcome,
            Reclassified {
                checked: 2,
                changed: 2,
                unanswered: 0
            }
        );
    }

    #[test]
    fn patterns_follow_the_host_preview_rules() {
        assert!(compile("receipt|invoice").is_some_and(|re| re.is_match("Your INVOICE")));
        assert!(compile("  ").is_none());
        assert!(compile("(").is_none());
        assert!(compile(&"a".repeat(301)).is_none());
    }
}
