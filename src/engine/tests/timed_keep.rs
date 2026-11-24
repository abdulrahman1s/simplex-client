//! Disappearing messages kept right: across a restart, a profile change, a removed friend (ChatTests/Direct.hs `testUsersTimedMessages`).

use crate::types::model::{ChatProfile, ChatRef, MessageBody};
use crate::types::{Event, Request};

use super::T;
use super::files::sent_events;
use super::timed::{later, send, set_timer, timed_msg};
use crate::chat::ChatEvent;
use crate::engine::agent_port::ConnId;

#[test]
fn due_items_go_after_a_restart_and_unread_ones_wait_to_be_read() {
    let mut t = T::started();
    let (bob, conn) = t.friend("bob");
    let chat = ChatRef::Direct(bob);
    set_timer(&mut t, bob, Some(30));
    let mine = send(&mut t, chat, "hi");
    let theirs = super::messages(&t.peer_says(&conn, 1, timed_msg("hey", Some(30))))[0].clone();
    let mut t = t.restarted();
    t.e.tick();
    let out = t.out();
    assert!(
        super::messages(&out)
            .iter()
            .any(|m| m.id == mine.id && m.body == MessageBody::Deleted),
        "came due while the client was off: {out:?}"
    );
    assert!(t.e.item(chat, theirs.id).unwrap().is_some());
    t.req(Request::MarkRead(chat));
    let mut t = t.restarted();
    t.e.tick();
    t.out();
    assert!(
        t.e.item(chat, theirs.id).unwrap().is_none(),
        "read, then due"
    );
}

fn infos_to(t: &T, conn: &ConnId) -> Vec<Option<i64>> {
    let sent = t.e.agent.sent();
    let to = sent.iter().filter(|(c, _)| c == conn);
    to.flat_map(|(_, b)| crate::chat::parse_chat_messages(b).unwrap())
        .filter_map(|m| match m.event {
            ChatEvent::Info { profile, .. } => Some(profile.preferences?.timed_messages?.ttl),
            _ => None,
        })
        .collect()
}

#[test]
fn a_profile_change_keeps_each_friends_own_timer() {
    let mut t = T::started();
    let (bob, bob_conn) = t.friend("bob");
    let (_, carol_conn) = t.friend("carol");
    set_timer(&mut t, bob, Some(60));
    t.req(Request::SetProfile(ChatProfile {
        name: "alice2".into(),
        ..Default::default()
    }));
    assert_eq!(infos_to(&t, &bob_conn), vec![Some(60), Some(60)]);
    assert_eq!(infos_to(&t, &carol_conn), vec![None]);
}

#[test]
fn removing_a_friend_drops_its_timers() {
    let mut t = T::started();
    let (bob, conn) = t.friend("bob");
    let chat = ChatRef::Direct(bob);
    set_timer(&mut t, bob, Some(30));
    send(&mut t, chat, "hi");
    t.peer_says(&conn, 1, timed_msg("hey", Some(30)));
    assert_eq!(t.e.state.timed.len(), 1);
    t.req(Request::RemoveFriend(bob));
    assert!(t.e.state.timed.is_empty());
    let mut left = |prefix: &[u8]| t.e.store.keys_with_prefix("chat", prefix).unwrap();
    assert!(left(b"tdue/").is_empty() && left(b"tread/").is_empty());
    later(&mut t, 60);
}

#[test]
fn a_timer_must_be_a_second_or_more() {
    let mut t = T::started();
    let (bob, _) = t.friend("bob");
    let out = set_timer(&mut t, bob, Some(0));
    assert!(out.iter().any(|e| matches!(e, Event::Failed { .. })));
    assert!(
        sent_events(&t)
            .iter()
            .all(|e| !matches!(e, ChatEvent::Info { .. }))
    );
}

#[test]
fn the_same_profile_again_does_not_undo_the_users_timer() {
    let mut t = T::started();
    let (bob, conn) = t.friend("bob");
    set_timer(&mut t, bob, Some(60));
    // Their profile as the handshake had it: no timer, nothing changed since.
    t.peer_says(&conn, 1, super::profile::their_info(None, |_| {}));
    t.peer_says(&conn, 2, super::profile::their_info(None, |_| {}));
    let mine = t.e.state.friends[&bob].my_timer.clone().unwrap();
    assert_eq!(mine.ttl, Some(60));
}
