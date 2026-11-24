//! A friend deletes its own message only if the relay took the deletion within 78 hours of the message (Subscriber.hs `messageDelete`, `rcvItemDeletable`).

use std::time::Duration;

use crate::types::model::MessageBody;

use super::{T, messages};
use crate::chat::ChatEvent;
use crate::chat::container::MsgContainer;
use crate::chat::ids::SharedMsgId;
use crate::engine::agent_port::ConnId;

const HOUR: Duration = Duration::from_secs(3600);

fn says(t: &mut T, conn: &ConnId, msg_id: i64) -> (i64, SharedMsgId) {
    let said = ChatEvent::MsgNew(MsgContainer::text("said"));
    let m = messages(&t.peer_says(conn, msg_id, said))[0].clone();
    let item = t.e.item(m.chat, m.id).unwrap().unwrap();
    (m.id, SharedMsgId(item.shared_id.unwrap()))
}

fn del(shared: SharedMsgId) -> ChatEvent {
    ChatEvent::MsgDel {
        msg_id: shared,
        member_id: None,
        scope: None,
        only_history: false,
    }
}

fn deletes(t: &mut T, conn: &ConnId, msg_id: i64, shared: SharedMsgId) -> Vec<crate::types::Event> {
    t.peer_says(conn, msg_id, del(shared))
}

#[test]
fn a_friend_deletes_its_message_within_78_hours_of_its_arrival() {
    let mut t = T::started();
    let (_, conn) = t.friend("bob");
    let (_, shared) = says(&mut t, &conn, 1);
    t.clock.advance(77 * HOUR);
    let m = messages(&deletes(&mut t, &conn, 2, shared))[0].clone();
    assert_eq!(m.body, MessageBody::Deleted);
}

#[test]
fn a_friends_deletion_of_a_message_older_than_78_hours_is_ignored() {
    let mut t = T::started();
    let (bob, conn) = t.friend("bob");
    let (id, shared) = says(&mut t, &conn, 1);
    t.clock.advance(78 * HOUR);
    assert!(messages(&deletes(&mut t, &conn, 2, shared)).is_empty());
    let kept =
        t.e.item(crate::types::model::ChatRef::Direct(bob), id)
            .unwrap()
            .unwrap();
    assert_eq!(kept.body, MessageBody::Text("said".into()));
}

#[test]
fn a_deletion_the_relay_took_in_time_counts_however_late_it_arrives() {
    use crate::chat::{ChatMessage, encode_chat_message};
    use crate::engine::agent_port::AgentEvent;
    use crate::engine::clock::Clock;
    let mut t = T::started();
    let (_, conn) = t.friend("bob");
    let (_, shared) = says(&mut t, &conn, 1);
    let deleted_at = t.clock.wall_ms() / 1000 + 3600;
    t.clock.advance(96 * HOUR);
    let msg = ChatMessage::new(del(shared)).unwrap();
    let out = t.agent(AgentEvent::Msg {
        conn,
        msg_id: 2,
        body: encode_chat_message(&msg, false).unwrap(),
        integrity: crate::agent::MsgIntegrity::Ok,
        broker_ts: deleted_at,
    });
    assert_eq!(messages(&out)[0].body, MessageBody::Deleted);
}
