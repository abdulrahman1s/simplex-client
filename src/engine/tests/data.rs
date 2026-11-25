//! Application data in a direct chat: sent as an unknown content type, heard as `Event::Data`, kept nowhere (Protocol.hs `MCUnknown`).

use serde_json::{Map, Value, json};

use crate::types::model::{ChatMessage, ChatRef, MessageBody};
use crate::types::{Event, Request, RequestKind};

use super::groups::messages::{failed, sent_to};
use super::{T, messages};
use crate::chat::ChatEvent;
use crate::chat::app_data;
use crate::chat::container::MsgContainer;
use crate::chat::content::MsgContent;

pub fn obj(v: Value) -> Map<String, Value> {
    let Value::Object(o) = v else { panic!() };
    o
}

/// Started with `tags` registered as data.
pub fn with_tags(tags: &[&str]) -> T {
    let mut t = T::new();
    let mut hello = t.dir.hello("alice");
    hello.data_tags = tags.iter().map(|s| (*s).to_owned()).collect();
    t.e.start(&hello).unwrap();
    t.out();
    t
}

pub fn history(t: &mut T, chat: ChatRef) -> Vec<ChatMessage> {
    let out = t.req(Request::History {
        chat,
        before: None,
        count: 50,
        tag: 9,
    });
    out.into_iter()
        .find_map(|e| match e {
            Event::History { messages, .. } => Some(messages),
            _ => None,
        })
        .unwrap()
}

pub fn send_data(chat: ChatRef, tag: &str, json: Value) -> Request {
    Request::SendData {
        chat,
        tag: tag.into(),
        json: obj(json),
        fallback_text: "an app message".into(),
    }
}

#[test]
fn sent_data_goes_as_an_unknown_content_type_and_makes_no_item() {
    let mut t = T::started();
    let (bob, conn) = t.friend("bob");
    let chat = ChatRef::Direct(bob);
    let before = history(&mut t, chat);
    let out = t.req(send_data(chat, "app.poll", json!({"q": "lunch?", "n": 2})));
    assert!(messages(&out).is_empty(), "{out:?}");
    assert!(!failed(&out, RequestKind::Send), "{out:?}");
    let Some(ChatEvent::MsgNew(c)) = sent_to(&t, &conn).pop() else {
        panic!()
    };
    let MsgContent::Unknown { tag, text, json } = c.content else {
        panic!("{:?}", c.content)
    };
    assert_eq!(
        (tag.as_str(), text.as_str()),
        ("app.poll", "an app message")
    );
    let want = json!({"type": "app.poll", "text": "an app message", "q": "lunch?", "n": 2});
    assert_eq!(Value::Object(json), want);
    assert_eq!(history(&mut t, chat), before);
}

#[test]
fn received_data_under_a_registered_tag_is_told_and_makes_no_item() {
    let mut t = with_tags(&["app.poll"]);
    let (bob, conn) = t.friend("bob");
    let before = history(&mut t, ChatRef::Direct(bob));
    let c = app_data::content("app.poll", &obj(json!({"q": "lunch?"})), "a poll");
    let out = t.peer_says(&conn, 1, ChatEvent::MsgNew(MsgContainer::simple(c)));
    assert!(messages(&out).is_empty(), "{out:?}");
    let data: Vec<_> = out
        .iter()
        .filter(|e| matches!(e, Event::Data { .. }))
        .collect();
    let want = Event::Data {
        chat: ChatRef::Direct(bob),
        member: None,
        tag: "app.poll".into(),
        json: obj(json!({"q": "lunch?"})),
    };
    assert_eq!(data, [&want]);
    assert_eq!(history(&mut t, ChatRef::Direct(bob)), before);
    assert_eq!(t.e.state.friends[&bob].unread, 0);
}

#[test]
fn an_unregistered_tag_still_arrives_as_its_fallback_text() {
    let mut t = with_tags(&["app.other"]);
    let (_, conn) = t.friend("bob");
    let c = app_data::content("app.poll", &obj(json!({"q": "lunch?"})), "a poll");
    let out = t.peer_says(&conn, 1, ChatEvent::MsgNew(MsgContainer::simple(c)));
    let [m] = messages(&out).try_into().unwrap();
    assert_eq!(m.body, MessageBody::Text("a poll".into()));
    assert!(!out.iter().any(|e| matches!(e, Event::Data { .. })));
}

#[test]
fn data_under_a_content_type_the_apps_know_or_with_type_or_text_is_refused() {
    let mut t = T::started();
    let (bob, conn) = t.friend("bob");
    let chat = ChatRef::Direct(bob);
    for r in [
        send_data(chat, "text", json!({})),
        send_data(chat, "", json!({})),
        send_data(chat, "app poll", json!({})),
        send_data(chat, "app.poll", json!({"text": "x"})),
        send_data(chat, "app.poll", json!({"type": "x"})),
    ] {
        assert!(failed(&t.req(r), RequestKind::Send));
    }
    assert!(sent_to(&t, &conn).is_empty());
}

#[test]
fn data_too_large_for_one_message_is_refused() {
    let mut t = T::started();
    let (bob, conn) = t.friend("bob");
    let big = "x".repeat(20_000);
    let out = t.req(send_data(
        ChatRef::Direct(bob),
        "app.blob",
        json!({"b": big}),
    ));
    assert!(failed(&out, RequestKind::Send));
    assert!(sent_to(&t, &conn).is_empty());
}
