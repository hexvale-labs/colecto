//! Drives `export!` the way an extension uses it, through the mock host.
#![forbid(unsafe_code)]

use colecto_extension::bundled::{composer, reader};
use colecto_extension::prelude::*;
use colecto_extension::testing::TestHost;
use serde_json::{Value, json};

#[derive(Serialize, Deserialize)]
struct Shout {
    draft: composer::v1::Draft,
}
#[derive(Serialize, Deserialize)]
struct Reply {
    draft: composer::v1::SetDraft,
}
colecto_extension::bus_request!(Shout => Reply, bus = "shouter", version = 1, name = "shout");

/// Answers with how many messages the account has, through a service call.
#[derive(Serialize, Deserialize)]
struct Count {}
colecto_extension::bus_request!(Count => usize, bus = "shouter", version = 1, name = "count");
fn count(_: Count, cx: &mut Context) -> Result<usize> {
    Ok(cx.messages()?.len())
}

fn shout(request: Shout, cx: &mut Context) -> Result<Reply> {
    if cx.sender() != Some("composer") {
        return Err(Error::other("only the composer may ask"));
    }
    let body = Some(request.draft.body.to_uppercase());
    Ok(Reply {
        draft: composer::v1::SetDraft {
            body,
            ..Default::default()
        },
    })
}

fn opened(event: reader::v1::MessageOpened, cx: &mut Context) -> Result<()> {
    cx.request(&reader::v1::AddBanner {
        message_id: event.message_id,
        text: "Seen".into(),
        tone: reader::v1::Tone::Info,
    })?;
    Ok(())
}

colecto_extension::export! {
    requests: [shout, count],
    events: [opened],
}

fn host(log: std::rc::Rc<std::cell::RefCell<Vec<Value>>>) -> TestHost {
    TestHost::new(move |request| {
        log.borrow_mut().push(request.clone());
        match request["operation"].as_str() {
            Some("messages.list") => {
                Ok(json!([{"id": "1", "newer_host_field": true}, {"id": "2"}]))
            }
            Some("bus.request") => Ok(Value::Null),
            _ => Err("Capability not granted".into()),
        }
    })
}

#[test]
fn service_calls_and_their_errors() {
    let count =
        json!({"event": "bus.request", "from": "x", "name": "v1/count", "payload": {}}).to_string();
    let mut host = host(Default::default());
    assert_eq!(
        host.run(colecto_main, &count),
        Some(json!({"bus": {"response": 2}}))
    );
    // A denied service call means no answer; the caller sees null.
    let mut denied = TestHost::new(|_| Err("Capability not granted".into()));
    assert_eq!(denied.run(colecto_main, &count), None);
    // Anything that is not a bus action does nothing.
    assert_eq!(host.run(colecto_main, "open"), None);
}

#[test]
fn answers_matching_bus_requests_only() {
    let mut host = host(Default::default());
    let action = |from: &str, name: &str| {
        json!({"event": "bus.request", "from": from, "name": name, "payload": {"draft": {"recipient": "", "subject": "", "body": "hi"}}}).to_string()
    };
    assert_eq!(
        host.run(colecto_main, &action("composer", "v1/shout")),
        Some(json!({"bus": {"response": {"draft": {"body": "HI"}}}}))
    );
    assert_eq!(
        host.run(colecto_main, &action("composer", "v2/shout")),
        None,
        "unknown name"
    );
    assert_eq!(
        host.run(colecto_main, &action("someone", "v1/shout")),
        None,
        "handler error"
    );
}

#[test]
fn routes_subscribed_events() {
    let log: std::rc::Rc<std::cell::RefCell<Vec<Value>>> = Default::default();
    let mut host = host(std::rc::Rc::clone(&log));
    let event = |bus: &str| {
        json!({"event": "bus.event", "bus": bus, "name": "v1/message_opened", "payload": {"message_id": "7", "subject": "S", "from": "a", "html": null, "text": "t"}}).to_string()
    };
    assert_eq!(
        host.run(colecto_main, &event("reader")),
        None,
        "events emit nothing"
    );
    assert_eq!(log.borrow().len(), 1);
    assert_eq!(
        log.borrow()[0],
        json!({"operation": "bus.request", "bus": "reader", "name": "v1/add_banner", "payload": {"message_id": "7", "text": "Seen", "tone": "info"}})
    );
    host.run(colecto_main, &event("other"));
    assert_eq!(
        log.borrow().len(),
        1,
        "same name on another bus is not routed"
    );
}

#[test]
fn bus_names_carry_versions() {
    assert_eq!(<Shout as BusRequest>::NAME, "v1/shout");
    assert_eq!(<composer::v1::GetDraft as BusRequest>::NAME, "v1/get_draft");
    assert_eq!(
        serde_json::to_value(composer::v1::GetDraft).unwrap(),
        Value::Null
    );
}

#[test]
fn unanswered_requests_are_none() {
    let mut host = host(Default::default());
    let draft = host.with(|cx| cx.request(&composer::v1::GetDraft));
    assert_eq!(draft, Ok(None));
}
