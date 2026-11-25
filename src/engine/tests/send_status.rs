//! A tagged request's sends, told against its tag: queued with the item or refused at once, then sent or failed per connection.

use crate::types::model::{ChatRef, SendStatus};
use crate::types::{ErrorCode, Event, Request};

use super::{T, messages};
use crate::chat::group::GroupMemberRole;
use crate::engine::agent_port::{AgentEvent, AgentFailure};

fn tagged(tag: u64, r: Request) -> Request {
    Request::Tagged {
        tag,
        request: Box::new(r),
    }
}

fn text(chat: ChatRef) -> Request {
    Request::Send {
        chat,
        text: "hi".into(),
        reply_to: None,
        mentions: Vec::new(),
    }
}

fn statuses(out: &[Event]) -> Vec<(u64, Option<i64>, SendStatus)> {
    out.iter()
        .filter_map(|e| match e {
            Event::SendStatus {
                tag,
                member,
                status,
            } => Some((*tag, *member, status.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn a_tagged_send_is_queued_with_its_item_then_sent() {
    let mut t = T::started();
    let (bob, conn) = t.friend("bob");
    let out = t.req(tagged(7, text(ChatRef::Direct(bob))));
    let item = messages(&out)[0].id;
    let queued = SendStatus::Queued { item: Some(item) };
    assert_eq!(statuses(&out), [(7, None, queued)]);
    let (&(c, msg_id), _) = t.e.send_tags.iter().next().unwrap();
    assert_eq!(c, conn);
    let out = t.agent(AgentEvent::Sent { conn, msg_id });
    assert_eq!(statuses(&out), [(7, None, SendStatus::Sent)]);
    assert!(t.e.send_tags.is_empty(), "the entry goes with its outcome");
}

#[test]
fn a_tagged_send_that_cannot_be_delivered_is_told_failed() {
    let mut t = T::started();
    let (bob, conn) = t.friend("bob");
    t.req(tagged(8, text(ChatRef::Direct(bob))));
    let (&(_, msg_id), _) = t.e.send_tags.iter().next().unwrap();
    let err = AgentFailure::Network("quota".into());
    let out = t.agent(AgentEvent::Merr { conn, msg_id, err });
    let failed = SendStatus::Failed {
        detail: "quota".into(),
    };
    assert_eq!(statuses(&out), [(8, None, failed)]);
}

#[test]
fn a_refused_tagged_send_is_told_against_its_tag_instead_of_as_failed() {
    let mut t = T::started();
    let out = t.req(tagged(9, text(ChatRef::Direct(404))));
    let told = statuses(&out);
    let [(9, None, SendStatus::Refused { code, .. })] = told.as_slice() else {
        panic!("{out:?}")
    };
    assert_eq!(*code, ErrorCode::RequestFailed);
    assert!(!out.iter().any(|e| matches!(e, Event::Failed { .. })));
    assert!(t.e.send_tags.is_empty());
}

#[test]
fn a_tagged_group_send_is_told_sent_per_member() {
    let mut t = T::started();
    let (g, ms) = t.group("team", &["bob", "carol"], GroupMemberRole::Member);
    t.req(tagged(10, text(ChatRef::Group(g))));
    assert_eq!(t.e.send_tags.len(), 2);
    let tags: Vec<_> = t.e.send_tags.keys().copied().collect();
    let mut told = Vec::new();
    for (conn, msg_id) in tags {
        told.extend(statuses(&t.agent(AgentEvent::Sent { conn, msg_id })));
    }
    told.sort_by_key(|(_, m, _)| *m);
    let want: Vec<_> = ms
        .iter()
        .map(|(m, _)| (10, Some(*m), SendStatus::Sent))
        .collect();
    assert_eq!(told, want);
}

#[test]
fn tagged_data_is_queued_without_an_item_and_an_untagged_send_tells_nothing() {
    let mut t = T::started();
    let (bob, _) = t.friend("bob");
    let data = super::data::send_data(ChatRef::Direct(bob), "app.x", serde_json::json!({}));
    let out = t.req(tagged(11, data));
    assert_eq!(
        statuses(&out),
        [(11, None, SendStatus::Queued { item: None })]
    );
    let out = t.req(text(ChatRef::Direct(bob)));
    assert!(statuses(&out).is_empty());
    assert_eq!(t.e.send_tags.len(), 1, "only the tagged one is kept");
}

#[test]
fn a_tagged_request_inside_another_is_refused() {
    let mut t = T::started();
    let (bob, _) = t.friend("bob");
    let out = t.req(tagged(12, tagged(13, text(ChatRef::Direct(bob)))));
    assert!(matches!(
        statuses(&out).as_slice(),
        [(12, None, SendStatus::Refused { .. })]
    ));
}
