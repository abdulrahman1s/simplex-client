//! A channel's owner on the fake agent: its relay made active only once the link data names it, posts as the channel, the signed size limit, and the relays it needs.

use crate::agent::links::ConnShortLink;
use crate::chat::container::MsgContainer;
use crate::chat::{ChatEvent, ChatMessage, parse_chat_elements};
use crate::engine::agent_port::AgentEvent;
use crate::engine::records::RelayStatus;
use crate::types::model::{ChatRef, ChatRelay};
use crate::types::{Event, Request};

use super::super::{Call, T};
use super::fixtures::*;

#[test]
fn an_owner_s_relay_is_active_only_once_the_link_data_naming_it_is_stored() {
    let mut t = T::started();
    let (group, member, link_conn) = owner_with_relay(&mut t);
    let status = |t: &T| {
        t.e.state.groups[&group]
            .channel
            .as_ref()
            .unwrap()
            .relay(member)
            .unwrap()
            .status
    };
    assert_eq!(status(&t), RelayStatus::AcknowledgedRoster);
    let published = (t.e.agent.calls.iter()).rev().find_map(|c| match c {
        Call::SetLinkData { relays, .. } => Some(relays.clone()),
        _ => None,
    });
    assert_eq!(
        published,
        Some(vec![ConnShortLink::parse(RELAYS[1]).unwrap().to_uri()])
    );
    t.agent(AgentEvent::LinkUpdated { conn: link_conn });
    assert_eq!(status(&t), RelayStatus::Active);
    // The roster went first, signed, with its blob under its id.
    let bodies: Vec<_> = t.e.agent.sent().into_iter().map(|(_, b)| b).collect();
    assert!(
        bodies.iter().any(|b| b.starts_with(b"/G")),
        "a signed roster header"
    );
    assert!(bodies.iter().any(|b| b.first() == Some(&b'F')), "its blob");
}

#[test]
fn an_owner_posts_as_the_channel_and_a_signed_post_too_large_is_refused_before_sending() {
    let mut t = T::started();
    let (group, _, link_conn) = owner_with_relay(&mut t);
    t.agent(AgentEvent::LinkUpdated { conn: link_conn });
    let before = t.e.agent.sent().len();
    t.req(Request::Send {
        chat: ChatRef::Group(group),
        text: "hello".into(),
        reply_to: None,
        mentions: Vec::new(),
    });
    let sent = t.e.agent.sent();
    assert_eq!(sent.len(), before + 1, "to the one relay only");
    let p = parse_chat_elements(&sent.last().unwrap().1)
        .remove(0)
        .unwrap();
    let ChatEvent::MsgNew(c) = p.message.event else {
        panic!()
    };
    assert_eq!(c.as_group, Some(true));
    // One byte over the signed limit once the signature's 69 bytes are on.
    let body = |n: usize| {
        let json = serde_json::Map::from_iter([("k".to_owned(), serde_json::json!("x".repeat(n)))]);
        let mut c = MsgContainer::simple(crate::chat::app_data::content(TAG, &json, ""));
        c.as_group = Some(true);
        let m = ChatMessage::new(ChatEvent::MsgNew(c)).unwrap();
        (json, m.to_json().unwrap().len())
    };
    let max = crate::limits::MAX_ENCODED_MSG_LENGTH;
    let (json, len) = body(max - 69 + 1 - body(0).1);
    assert_eq!(len + 69, max + 1);
    let out = t.req(Request::SendSignedData {
        chat: ChatRef::Group(group),
        tag: TAG.into(),
        json,
        fallback_text: String::new(),
    });
    assert!(
        out.iter().any(
            |e| matches!(e, Event::Failed { detail, .. } if detail == "signed message too large")
        ),
        "{out:?}"
    );
    assert_eq!(t.e.agent.sent().len(), before + 1, "nothing more is sent");
}

#[test]
fn a_new_channel_needs_the_caller_s_relays() {
    let mut t = T::started();
    let out = t.req(Request::NewChannel("news".into()));
    assert!(
        out.iter().any(|e| matches!(e, Event::Failed { .. })),
        "{out:?}"
    );
    assert!(t.e.state.groups.is_empty());
}

#[test]
fn the_largest_signed_post_a_relay_takes_is_sent() {
    let mut t = T::started();
    let (group, _, link_conn) = owner_with_relay(&mut t);
    t.agent(AgentEvent::LinkUpdated { conn: link_conn });
    let body = |n: usize| {
        let json = serde_json::Map::from_iter([("k".to_owned(), serde_json::json!("x".repeat(n)))]);
        let mut c = MsgContainer::simple(crate::chat::app_data::content(TAG, &json, ""));
        c.as_group = Some(true);
        let m = ChatMessage::new(ChatEvent::MsgNew(c)).unwrap();
        (json, m.to_json().unwrap().len())
    };
    let max = crate::limits::MAX_ENCODED_MSG_LENGTH;
    let (json, _) = body(max - 69 - body(0).1);
    let before = t.e.agent.sent().len();
    t.req(Request::SendSignedData {
        chat: ChatRef::Group(group),
        tag: TAG.into(),
        json,
        fallback_text: String::new(),
    });
    let sent = t.e.agent.sent();
    assert_eq!(sent.len(), before + 1);
    assert_eq!(sent.last().unwrap().1.len(), max);
}

#[test]
fn preset_chat_relays_parse_and_are_never_used_unless_passed() {
    for r in ChatRelay::presets() {
        assert!(
            matches!(
                ConnShortLink::parse(&r.address),
                Ok(ConnShortLink::Contact { .. })
            ),
            "{}",
            r.address
        );
    }
    let t = T::started();
    assert!(t.e.state.settings.chat_relays.is_empty());
}
