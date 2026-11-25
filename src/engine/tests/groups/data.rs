//! Application data in a group: to every member, told with its sender, not told for a blocked member.

use serde_json::json;

use crate::types::Event;
use crate::types::Request;
use crate::types::model::ChatRef;

use super::messages::{says, sent_to};
use crate::chat::ChatEvent;
use crate::chat::app_data;
use crate::chat::container::MsgContainer;
use crate::chat::content::MsgContent;
use crate::chat::group::GroupMemberRole;
use crate::engine::tests::data::{history, obj, send_data, with_tags};
use crate::engine::tests::messages;

#[test]
fn group_data_goes_to_every_member_and_makes_no_item() {
    let mut t = with_tags(&[]);
    let (g, ms) = t.group("team", &["bob", "carol"], GroupMemberRole::Member);
    let out = t.req(send_data(ChatRef::Group(g), "app.poll", json!({"q": 1})));
    assert!(messages(&out).is_empty(), "{out:?}");
    for (_, conn) in &ms {
        let Some(ChatEvent::MsgNew(c)) = sent_to(&t, conn).pop() else {
            panic!()
        };
        assert!(matches!(c.content, MsgContent::Unknown { ref tag, .. } if tag == "app.poll"));
    }
    assert!(history(&mut t, ChatRef::Group(g)).is_empty());
}

#[test]
fn group_data_is_told_with_its_member_unless_they_are_blocked() {
    let mut t = with_tags(&["app.poll"]);
    let (g, ms) = t.group("team", &["bob", "carol"], GroupMemberRole::Member);
    let c = || {
        MsgContainer::simple(app_data::content(
            "app.poll",
            &obj(json!({"q": 1})),
            "a poll",
        ))
    };
    let out = says(&mut t, &ms[1].1, 1, c());
    let want = Event::Data {
        chat: ChatRef::Group(g),
        member: Some(ms[1].0),
        tag: "app.poll".into(),
        json: obj(json!({"q": 1})),
    };
    assert!(out.contains(&want), "{out:?}");
    assert!(messages(&out).is_empty());
    t.req(Request::SetMemberBlocked {
        group: g,
        member: ms[1].0,
        on: true,
    });
    let out = says(&mut t, &ms[1].1, 2, c());
    assert!(
        !out.iter().any(|e| matches!(e, Event::Data { .. })),
        "{out:?}"
    );
    assert!(messages(&out).is_empty());
    assert!(history(&mut t, ChatRef::Group(g)).is_empty());
}
