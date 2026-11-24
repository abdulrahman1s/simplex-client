//! A broken message chain and a receipt with a bad hash are shown, as `checkIntegrityCreateItem` and `CISSndRcvd MRBadMsgHash` show them.

use crate::types::Request;
use crate::types::model::{
    ChatNotice, ChatRef, Delivery, IntegrityError, MessageAuthor, MessageBody,
};

use super::{T, messages};
use crate::agent::client::RcptStatus;
use crate::agent::{MsgErrorType, MsgIntegrity};
use crate::chat::container::MsgContainer;
use crate::chat::group::GroupMemberRole;
use crate::chat::{ChatEvent, ChatMessage, encode_chat_message};
use crate::engine::agent_port::{AgentEvent, ConnId};

fn text(t: &mut T, conn: ConnId, msg_id: i64, integrity: MsgIntegrity) -> Vec<crate::types::Event> {
    let msg = ChatMessage::new(ChatEvent::MsgNew(MsgContainer::text("m3"))).unwrap();
    t.agent(AgentEvent::Msg {
        conn,
        msg_id,
        body: encode_chat_message(&msg, false).unwrap(),
        integrity,
    })
}

#[test]
fn a_skipped_message_is_told_before_the_message_that_follows_the_gap() {
    let mut t = T::started();
    let (_, conn) = t.friend("bob");
    let skipped = MsgIntegrity::Error(MsgErrorType::Skipped { from: 2, to: 2 });
    let shown = messages(&text(&mut t, conn, 3, skipped));
    let bodies: Vec<_> = shown.iter().map(|m| m.body.clone()).collect();
    assert_eq!(
        bodies,
        [
            MessageBody::Notice(ChatNotice::Integrity(IntegrityError::Skipped {
                from: 2,
                to: 2
            })),
            MessageBody::Text("m3".into()),
        ]
    );
    assert_eq!(shown[0].author, MessageAuthor::Friend);
}

#[test]
fn a_bad_hash_from_a_member_is_told_in_the_group_under_their_name() {
    let mut t = T::started();
    let (_, ms) = t.group("team", &["bob"], GroupMemberRole::Member);
    let bad = MsgIntegrity::Error(MsgErrorType::BadHash);
    let shown = messages(&text(&mut t, ms[0].1, 1, bad));
    assert_eq!(
        (shown[0].author.clone(), shown[0].body.clone()),
        (
            MessageAuthor::Member("bob".into()),
            MessageBody::Notice(ChatNotice::Integrity(IntegrityError::BadHash))
        )
    );
    assert_eq!(shown.len(), 2, "the message itself is still shown");
}

#[test]
fn a_message_that_follows_the_chain_adds_no_notice() {
    let mut t = T::started();
    let (_, conn) = t.friend("bob");
    assert_eq!(messages(&text(&mut t, conn, 1, MsgIntegrity::Ok)).len(), 1);
}

#[test]
fn a_receipt_with_a_bad_hash_shows_the_message_delivered_with_a_bad_hash() {
    let mut t = T::started();
    let (bob, conn) = t.friend("bob");
    t.req(Request::Send {
        chat: ChatRef::Direct(bob),
        text: "gg".into(),
        reply_to: None,
        mentions: Vec::new(),
    });
    let out = t.agent(AgentEvent::Rcvd {
        conn,
        msg_id: 1,
        status: RcptStatus::BadMsgHash,
    });
    assert_eq!(messages(&out)[0].delivery, Delivery::DeliveredBadHash);
}

#[test]
fn one_members_bad_hash_receipt_wins_over_the_others_good_ones() {
    let mut t = T::started();
    let (g, ms) = t.group("team", &["bob", "carol"], GroupMemberRole::Owner);
    t.req(Request::Send {
        chat: ChatRef::Group(g),
        text: "hello".into(),
        reply_to: None,
        mentions: Vec::new(),
    });
    // Each member's connection carries its own agent message: 1 for bob, 2 for carol.
    let rcvd = |t: &mut T, conn, msg_id, status| {
        messages(&t.agent(AgentEvent::Rcvd {
            conn,
            msg_id,
            status,
        }))
    };
    let bad = rcvd(&mut t, ms[0].1, 1, RcptStatus::BadMsgHash);
    assert_eq!(bad[0].delivery, Delivery::DeliveredBadHash);
    assert!(rcvd(&mut t, ms[1].1, 2, RcptStatus::Ok).is_empty());
    let mut t = T::started();
    let (g, ms) = t.group("team", &["bob", "carol"], GroupMemberRole::Owner);
    t.req(Request::Send {
        chat: ChatRef::Group(g),
        text: "hello".into(),
        reply_to: None,
        mentions: Vec::new(),
    });
    assert_eq!(
        rcvd(&mut t, ms[0].1, 1, RcptStatus::Ok)[0].delivery,
        Delivery::Delivered
    );
    let bad = rcvd(&mut t, ms[1].1, 2, RcptStatus::BadMsgHash);
    assert_eq!(
        bad[0].delivery,
        Delivery::DeliveredBadHash,
        "after a good one too"
    );
}
