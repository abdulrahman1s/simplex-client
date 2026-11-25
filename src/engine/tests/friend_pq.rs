//! A friend's connection made without PQ gets it once they send, and their range is kept (Internal.hs `updatePeerChatVRange`; Agent.hs `sendMessage`).

use crate::types::Request;
use crate::types::model::ChatRef;

use super::T;
use crate::chat::ChatEvent;
use crate::chat::container::MsgContainer;
use crate::chat::group::GroupMemberRole;
use crate::chat::version::CURRENT_CHAT_VERSION;
use crate::engine::records::{FriendRecord, decode, encode};

fn text(chat: ChatRef) -> Request {
    Request::Send {
        chat,
        text: "hi".into(),
        reply_to: None,
        mentions: Vec::new(),
    }
}

fn last_pq(t: &T) -> bool {
    t.e.agent.pq_sent.last().unwrap().1
}

#[test]
fn a_friend_without_pq_is_sent_to_with_pq_once_they_send_a_message() {
    let mut t = T::started();
    let (bob, conn) = t.friend("bob");
    let mut f = t.e.friend(bob).unwrap();
    f.pq = false;
    t.e.save_friend(f).unwrap();
    t.req(text(ChatRef::Direct(bob)));
    assert!(!last_pq(&t));
    t.peer_says(&conn, 1, ChatEvent::MsgNew(MsgContainer::text("hello")));
    let f = t.e.friend(bob).unwrap();
    assert!(f.pq);
    assert_eq!(f.chat_v.map(|v| v.max), Some(CURRENT_CHAT_VERSION));
    t.req(text(ChatRef::Direct(bob)));
    assert!(last_pq(&t));
}

#[test]
fn a_friend_record_from_before_pq_was_kept_reads_as_pq_on() {
    let rec = FriendRecord::new(1, crate::engine::agent_port::ConnId(1), 0);
    let mut json: serde_json::Value = serde_json::from_slice(&encode(&rec).unwrap()).unwrap();
    let r = json["r"].as_object_mut().unwrap();
    r.remove("pq");
    r.remove("chat_v");
    let old: FriendRecord = decode(&serde_json::to_vec(&json).unwrap()).unwrap();
    assert!(old.pq);
    assert_eq!(old.chat_v, None);
}

#[test]
fn group_messages_go_without_pq() {
    let mut t = T::started();
    let (g, _) = t.group("team", &["bob", "carol"], GroupMemberRole::Owner);
    t.req(text(ChatRef::Group(g)));
    assert!(!t.e.agent.pq_sent.is_empty());
    assert!(t.e.agent.pq_sent.iter().all(|(_, pq)| !pq));
}

fn listed_pq(out: &[crate::types::Event], id: i64) -> Option<bool> {
    out.iter().rev().find_map(|e| match e {
        crate::types::Event::Chats { friends, .. } => {
            friends.iter().find(|f| f.id == id).map(|f| f.pq)
        }
        _ => None,
    })
}

#[test]
fn a_friends_pq_is_what_the_ratchet_negotiated_and_false_with_a_classical_peer() {
    let mut t = T::started();
    let (bob, bob_conn) = t.friend("bob");
    t.e.agent.classical.push(bob_conn);
    let (carol, carol_conn) = t.friend("carol");
    assert!(t.e.friend(bob).unwrap().pq, "PQ was asked for with both");
    t.peer_says(&bob_conn, 1, ChatEvent::MsgNew(MsgContainer::text("hi")));
    let out = t.peer_says(&carol_conn, 1, ChatEvent::MsgNew(MsgContainer::text("hi")));
    let out = [out, t.req(Request::Refresh)].concat();
    assert_eq!(listed_pq(&out, bob), Some(false));
    assert_eq!(listed_pq(&out, carol), Some(true));
    t.e.agent.classical.push(carol_conn);
    let out = t.peer_says(
        &carol_conn,
        2,
        ChatEvent::MsgNew(MsgContainer::text("again")),
    );
    assert_eq!(
        listed_pq(&out, carol),
        Some(false),
        "the list goes out again"
    );
}

#[test]
fn a_friends_pq_is_read_from_the_ratchet_at_the_start() {
    let mut t = T::started();
    let (bob, _) = t.friend("bob");
    let mut f = t.e.friend(bob).unwrap();
    f.pq_on = false;
    t.e.save_friend(f).unwrap();
    let t = t.restarted();
    assert!(
        t.e.friend(bob).unwrap().pq_on,
        "the ratchet says so before any message"
    );
}
