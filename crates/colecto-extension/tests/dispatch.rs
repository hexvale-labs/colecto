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

fn panel(event: PanelEvent, cx: &mut Context) -> Result<View> {
    let title = match event {
        PanelEvent::Open => "Opened".to_string(),
        PanelEvent::Action(label) => label,
    };
    let count = cx.messages()?.len();
    Ok(View::new(title, format!("{count} messages")).action("Refresh"))
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
    panel: panel,
    requests: [shout],
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
fn panel_views_and_errors() {
    let log = Default::default();
    let mut host = host(std::rc::Rc::clone(&log));
    let view = host.run(colecto_main, "open").unwrap();
    assert_eq!(
        view,
        json!({"title": "Opened", "text": "2 messages", "actions": ["Refresh"]})
    );
    assert_eq!(
        host.run(colecto_main, "Refresh").unwrap()["title"],
        "Refresh"
    );

    let mut denied = TestHost::new(|_| Err("Capability not granted".into()));
    let view = denied.run(colecto_main, "open").unwrap();
    assert_eq!(
        (view["title"].as_str(), view["text"].as_str()),
        (Some("Error"), Some("Capability not granted"))
    );
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
