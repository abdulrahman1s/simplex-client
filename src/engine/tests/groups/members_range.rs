//! A member's chat range follows each JSON message they send, not a binary file chunk (Internal.hs `updateMemberChatVRange`, `saveGroupRcvMsg`).

use super::super::T;
use super::messages::says;
use crate::chat::container::MsgContainer;
use crate::chat::element::encode_file_chunk_element;
use crate::chat::event::FileChunk;
use crate::chat::group::GroupMemberRole;
use crate::chat::ids::SharedMsgId;
use crate::chat::version::{SUPPORTED_CHAT_VRANGE, VersionRange};
use crate::engine::agent_port::AgentEvent;

const OLD: VersionRange = VersionRange::new(9, 17);

fn chat_v(t: &T, g: i64, m: i64) -> Option<VersionRange> {
    t.e.state.members[&(g, m)].chat_v
}

/// As a member who connected before upgrading is stored.
fn stored_at_old(t: &mut T, g: i64, m: i64) {
    let mut rec = t.e.state.members[&(g, m)].clone();
    rec.chat_v = Some(OLD);
    t.e.save_member(rec).unwrap();
}

#[test]
fn a_member_who_upgraded_is_seen_to_from_their_next_message() {
    let mut t = T::started();
    let (g, ms) = t.group("team", &["bob"], GroupMemberRole::Member);
    let (bob, conn) = ms[0];
    stored_at_old(&mut t, g, bob);
    says(&mut t, &conn, 1, MsgContainer::text("upgraded"));
    assert_eq!(chat_v(&t, g, bob), Some(SUPPORTED_CHAT_VRANGE));
    let t = t.restarted();
    assert_eq!(chat_v(&t, g, bob), Some(SUPPORTED_CHAT_VRANGE), "stored");
}

#[test]
fn a_file_chunk_leaves_the_range_alone() {
    let mut t = T::started();
    let (g, ms) = t.group("team", &["bob"], GroupMemberRole::Member);
    let (bob, conn) = ms[0];
    stored_at_old(&mut t, g, bob);
    let chunk = FileChunk::Chunk {
        chunk_no: 1,
        bytes: b"abc".to_vec(),
    };
    let body = encode_file_chunk_element(&SharedMsgId::mint().unwrap(), &chunk).unwrap();
    t.agent(AgentEvent::Msg {
        conn,
        msg_id: 1,
        body,
        integrity: crate::agent::MsgIntegrity::Ok,
    });
    assert_eq!(chat_v(&t, g, bob), Some(OLD));
}
