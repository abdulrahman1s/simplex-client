//! Messages for members still connecting wait for their CON (Internal.hs `memberSendAction`, `sendPendingGroupMessages`; Subscriber.hs CON).

use std::time::Duration;

use crate::types::model::{ChatRef, Delivery, MessageBody};
use crate::types::{Request, RequestKind};

use super::members::{acpt, invited_bob};
use super::members_support::{link, sent_on};
use super::messages::{failed, member_id, send};
use crate::chat::ChatEvent;
use crate::chat::group::{GroupMemberRole, IntroInvitation};
use crate::engine::agent_port::{AgentEvent, ConnId};
use crate::engine::records::{MemberCategory, MemberStatus, NS, keys};
use crate::engine::tests::T;

/// alice's "team" with bob invited and his CONF in: a member still connecting.
fn bob_connecting(t: &mut T) -> (i64, i64, ConnId) {
    let (g, bob, conn, _) = invited_bob(t);
    accepted(t, g, bob, conn)
}

fn accepted(t: &mut T, g: i64, m: i64, conn: ConnId) -> (i64, i64, ConnId) {
    let info = acpt(t, g, m, "bob");
    t.agent(AgentEvent::Conf {
        conn,
        conn_info: info,
    });
    assert_eq!(t.e.state.members[&(g, m)].status, MemberStatus::Accepted);
    (g, m, conn)
}

/// Friend bob added to `g` and his CONF in: ((group, member, member conn), friend).
fn bob_connecting_in(t: &mut T, g: i64) -> ((i64, i64, ConnId), i64) {
    let (bf, _) = t.friend("bob");
    t.req(Request::AddMember {
        group: g,
        friend: bf,
    });
    let m =
        t.e.state
            .members_of(g)
            .find(|m| m.friend == Some(bf))
            .unwrap()
            .clone();
    let conn = m.conn.unwrap();
    t.agent(AgentEvent::Invitation {
        conn,
        full: link(1),
        short: None,
    });
    (accepted(t, g, m.id, conn), bf)
}

fn texts(events: &[ChatEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| match e {
            ChatEvent::MsgNew(c) => Some(c.content.text().to_owned()),
            _ => None,
        })
        .collect()
}

/// The fake agent numbers every send in turn, from 1.
fn agent_msg_id(t: &T, conn: ConnId) -> i64 {
    let n =
        t.e.agent
            .sent()
            .iter()
            .position(|(c, _)| *c == conn)
            .unwrap();
    i64::try_from(n).unwrap() + 1
}

#[test]
fn message_sent_while_member_connects_is_delivered_on_con() {
    let mut t = T::started();
    let (g, bob, conn) = bob_connecting(&mut t);
    t.req(send(g, "hello", vec![]));
    assert!(sent_on(&t, conn).is_empty());
    t.agent(AgentEvent::Con { conn });
    let got = sent_on(&t, conn);
    assert!(
        matches!(got[0], ChatEvent::MsgNew(_)),
        "what waited goes first"
    );
    assert_eq!(texts(&got), ["hello"]);
    assert!(t.e.store.get(NS, &keys::pending(g, bob)).unwrap().is_none());
    // Sent once it went out, like any item.
    let msg_id = agent_msg_id(&t, conn);
    t.agent(AgentEvent::Sent { conn, msg_id });
    let page = t.e.items_page(ChatRef::Group(g), None, 10).unwrap();
    let hello = page
        .iter()
        .find(|m| m.body == MessageBody::Text("hello".into()))
        .unwrap();
    assert_eq!(hello.delivery, Delivery::Sent);
}

#[test]
fn a_waiting_message_survives_a_restart_and_goes_once() {
    let mut t = T::started();
    let (g, _, conn) = bob_connecting(&mut t);
    t.req(send(g, "hello", vec![]));
    let mut t = t.restarted();
    t.agent(AgentEvent::Con { conn });
    t.agent(AgentEvent::Con { conn });
    assert_eq!(texts(&sent_on(&t, conn)), ["hello"]);
}

#[test]
fn a_message_waits_30_days_at_most() {
    let mut t = T::started();
    let (g, _, conn) = bob_connecting(&mut t);
    t.req(send(g, "old", vec![]));
    t.clock.advance(Duration::from_secs(29 * 24 * 3600));
    t.req(send(g, "new", vec![]));
    t.clock.advance(Duration::from_secs(2 * 24 * 3600));
    t.agent(AgentEvent::Con { conn });
    assert_eq!(texts(&sent_on(&t, conn)), ["new"]);
}

#[test]
fn what_waited_goes_with_the_members_connection() {
    let mut t = T::started();
    let (g, bob, _) = bob_connecting(&mut t);
    t.req(send(g, "hello", vec![]));
    assert!(t.e.store.get(NS, &keys::pending(g, bob)).unwrap().is_some());
    t.e.drop_member_conn(g, bob, false).unwrap();
    assert!(t.e.store.get(NS, &keys::pending(g, bob)).unwrap().is_none());
}

#[test]
fn x_grp_mem_fwd_for_a_member_still_connecting_is_delivered_on_its_con() {
    let mut t = T::started();
    let (g, members) = t.group("team", &["carol"], GroupMemberRole::Owner);
    let (carol, carol_conn) = members[0];
    let ((_, bob, conn), _) = bob_connecting_in(&mut t, g);
    t.e.state.members.get_mut(&(g, carol)).unwrap().status = MemberStatus::Accepted;
    t.agent(AgentEvent::Con { conn });
    let intro = IntroInvitation {
        group_conn_req: link(2),
    };
    let carol_id = member_id(&t, g, carol);
    t.peer_says(
        &conn,
        1,
        ChatEvent::GrpMemInv {
            member_id: carol_id,
            intro: intro.clone(),
        },
    );
    assert!(sent_on(&t, carol_conn).is_empty());
    t.agent(AgentEvent::Con { conn: carol_conn });
    let got = sent_on(&t, carol_conn);
    let [
        ChatEvent::GrpMemNew {
            member_info: new, ..
        },
        ChatEvent::GrpMemFwd {
            member_info,
            intro: i,
        },
        ..,
    ] = got.as_slice()
    else {
        panic!("{got:?}")
    };
    let bob_id = member_id(&t, g, bob);
    assert_eq!(
        (&new.member_id, &member_info.member_id, i),
        (&bob_id, &bob_id, &intro)
    );
}

#[test]
fn a_message_the_announcer_forwards_to_a_post_member_does_not_wait() {
    let mut t = T::started();
    let (g, members) = t.group("team", &["bob", "carol"], GroupMemberRole::Member);
    let (host, carol) = (members[0].0, members[1].0);
    let mut c = t.e.state.members[&(g, carol)].clone();
    (c.category, c.status, c.conn, c.invited_by) = (
        MemberCategory::Post,
        MemberStatus::Announced,
        None,
        Some(host),
    );
    t.e.save_member(c).unwrap();
    t.req(send(g, "hi", vec![]));
    assert!(
        t.e.store
            .get(NS, &keys::pending(g, carol))
            .unwrap()
            .is_none()
    );
    // Without its announcer among the recipients it waits.
    t.e.state.members.get_mut(&(g, host)).unwrap().status = MemberStatus::Left;
    t.req(send(g, "hi again", vec![]));
    assert!(
        t.e.store
            .get(NS, &keys::pending(g, carol))
            .unwrap()
            .is_some()
    );
}

#[test]
fn nothing_waits_for_a_member_who_is_gone() {
    let mut t = T::started();
    let (g, members) = t.group("team", &["carol"], GroupMemberRole::Owner);
    let carol = members[0].0;
    let ((_, _, conn), _) = bob_connecting_in(&mut t, g);
    t.agent(AgentEvent::Con { conn });
    t.e.drop_member_conn(g, carol, false).unwrap();
    t.e.state.members.get_mut(&(g, carol)).unwrap().status = MemberStatus::Left;
    let intro = IntroInvitation {
        group_conn_req: link(2),
    };
    let carol_id = member_id(&t, g, carol);
    t.peer_says(
        &conn,
        1,
        ChatEvent::GrpMemInv {
            member_id: carol_id,
            intro,
        },
    );
    assert!(
        t.e.store
            .get(NS, &keys::pending(g, carol))
            .unwrap()
            .is_none()
    );
}

#[test]
fn the_user_still_joining_sends_nothing_and_keeps_nothing() {
    let mut t = T::started();
    let (g, members) = t.group("team", &["alice"], GroupMemberRole::Member);
    let host = members[0].0;
    let mut gr = t.e.state.groups[&g].clone();
    gr.status = MemberStatus::Accepted;
    t.e.save_group(gr).unwrap();
    t.e.state.members.get_mut(&(g, host)).unwrap().status = MemberStatus::Accepted;
    let out = t.req(send(g, "too early", vec![]));
    assert!(failed(&out, RequestKind::Send));
    assert!(
        t.e.store
            .get(NS, &keys::pending(g, host))
            .unwrap()
            .is_none()
    );
}
