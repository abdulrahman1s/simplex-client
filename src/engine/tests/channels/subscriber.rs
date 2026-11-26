//! A channel's subscriber on the fake agent: its join from link to first post, two relays' copies told once, and the observer's refusal.

use crate::agent::links::ConnShortLink;
use crate::chat::container::MsgContainer;
use crate::chat::group::GroupMemberRole;
use crate::chat::ids::{MemberId, b64_decode};
use crate::chat::signed::ChatBinding;
use crate::chat::{ChatEvent, parse_chat_elements};
use crate::crypto::{Alg, PublicKey, Signature};
use crate::engine::agent_port::AgentEvent;
use crate::engine::records::MemberStatus;
use crate::types::model::{ChatRef, MessageAuthor};
use crate::types::{Event, Request};

use super::super::{Call, T};
use super::fixtures::*;

#[test]
fn a_subscriber_joins_each_relay_with_a_signed_member_key_and_pq_off() {
    let mut t = T::started();
    let ch = Channel::new();
    let (group, _) = subscribed(&mut t, &ch);
    let first = ConnShortLink::parse(&lookups(&t)[0]).unwrap();
    assert_eq!(
        first.link_key(),
        ConnShortLink::parse(CHANNEL).unwrap().link_key()
    );
    assert_eq!(
        lookups(&t).len(),
        3,
        "the channel's link, then each relay's"
    );
    let joins: Vec<_> = (t.e.agent.calls.iter())
        .filter_map(|c| match c {
            Call::Join { conn_info, .. } => Some(conn_info.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(joins.len(), 2);
    assert!(
        t.e.agent.pq_seen.iter().all(|(_, pq)| !pq),
        "channel legs are PQ off"
    );
    let g = t.e.state.groups[&group].clone();
    let p = parse_chat_elements(&joins[0]).remove(0).unwrap();
    let sm = p.signed.unwrap();
    assert_eq!(sm.binding, ChatBinding::Group);
    let ChatEvent::Member {
        new_member_id,
        new_member_key,
        via_relay,
        ..
    } = p.message.event
    else {
        panic!()
    };
    assert_eq!(new_member_id, g.my_member_id);
    assert_eq!(via_relay, Some(MemberId(b"relay-member-0".to_vec())));
    // Signed over `'G' ‖ smpEncode (publicGroupId, memberId)`, under the key it asserts.
    let key = PublicKey::from_der(&b64_decode(&new_member_key).unwrap()).unwrap();
    let mut binding = vec![32];
    binding.extend_from_slice(&ch.id);
    binding.push(u8::try_from(g.my_member_id.0.len()).unwrap());
    binding.extend_from_slice(&g.my_member_id.0);
    let sig = Signature::from_raw(Alg::Ed25519, &sm.signatures[0].signature).unwrap();
    assert!(key.verify(&sig, &sm.signed_input(&binding)));
    assert_eq!(g.status, MemberStatus::Connected);
    assert_eq!(g.my_role, GroupMemberRole::Observer);
}

#[test]
fn a_post_as_the_channel_through_two_relays_is_told_once() {
    let mut t = T::started();
    let ch = Channel::new();
    let (group, conns) = subscribed(&mut t, &ch);
    let mut text = MsgContainer::text("hello");
    text.as_group = Some(true);
    let mut told = Vec::new();
    for (i, conn) in conns.iter().enumerate() {
        let msg = i64::try_from(i).unwrap() * 2 + 10;
        told.extend(t.agent(AgentEvent::Msg {
            conn: *conn,
            msg_id: msg,
            body: data_post(b"post-one"),
            integrity: crate::agent::MsgIntegrity::Ok,
            broker_ts: 0,
        }));
        told.extend(t.agent(AgentEvent::Msg {
            conn: *conn,
            msg_id: msg + 1,
            body: post(b"post-two", text.clone()),
            integrity: crate::agent::MsgIntegrity::Ok,
            broker_ts: 0,
        }));
    }
    let data = told
        .iter()
        .filter(|e| matches!(e, Event::Data { member: None, .. }))
        .count();
    assert_eq!(data, 1, "one Data for two relays' copies");
    let texts: Vec<_> = (told.iter())
        .filter_map(|e| match e {
            Event::Messages(m) => Some(m.clone()),
            _ => None,
        })
        .flatten()
        .filter(|m| {
            m.chat == ChatRef::Group(group) && matches!(m.author, MessageAuthor::Channel(_))
        })
        .collect();
    assert_eq!(texts.len(), 1, "{texts:?}");
}

#[test]
fn an_observer_s_post_is_refused_with_the_role_it_needs() {
    let mut t = T::started();
    let ch = Channel::new();
    let (group, _) = subscribed(&mut t, &ch);
    let sent = t.e.agent.sent().len();
    let out = t.req(Request::Send {
        chat: ChatRef::Group(group),
        text: "hi".into(),
        reply_to: None,
        mentions: Vec::new(),
    });
    assert!(
        out.iter().any(|e| matches!(e, Event::Failed { detail, .. } if detail == "groupUserRole: required role author")),
        "{out:?}"
    );
    assert_eq!(t.e.agent.sent().len(), sent, "nothing is sent");
}
