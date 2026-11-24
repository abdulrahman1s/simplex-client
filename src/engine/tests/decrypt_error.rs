//! A message that did not decrypt is told once, and the same error again counts on the same notice (Subscriber.hs `mdeUpdatedCI`).

use crate::types::Request;
use crate::types::model::{ChatNotice, DecryptError, MessageAuthor, MessageBody};

use super::{T, messages};
use crate::chat::group::GroupMemberRole;
use crate::engine::agent_port::{AgentEvent, ConnId};

fn fails(t: &mut T, conn: ConnId, error: DecryptError, count: u32) -> Vec<crate::types::Event> {
    t.agent(AgentEvent::DecryptError { conn, error, count })
}

fn shown(error: DecryptError, count: u32) -> MessageBody {
    MessageBody::Notice(ChatNotice::DecryptionError { error, count })
}

#[test]
fn messages_that_do_not_decrypt_are_counted_on_one_notice() {
    let mut t = T::started();
    let (_, conn) = t.friend("bob");
    let first = messages(&fails(&mut t, conn, DecryptError::Header, 1));
    assert_eq!(first[0].body, shown(DecryptError::Header, 1));
    let again = messages(&fails(&mut t, conn, DecryptError::Header, 1));
    assert_eq!(
        (again[0].id, again[0].body.clone()),
        (first[0].id, shown(DecryptError::Header, 2))
    );
    let other = messages(&fails(&mut t, conn, DecryptError::Earlier, 1));
    assert_ne!(other[0].id, first[0].id, "another error is another notice");
}

#[test]
fn too_many_skipped_shows_the_latest_count_not_a_sum() {
    let mut t = T::started();
    let (_, conn) = t.friend("bob");
    fails(&mut t, conn, DecryptError::TooManySkipped, 600);
    let m = messages(&fails(&mut t, conn, DecryptError::TooManySkipped, 601));
    assert_eq!(m[0].body, shown(DecryptError::TooManySkipped, 601));
}

#[test]
fn a_members_decryption_error_is_told_under_their_name() {
    let mut t = T::started();
    let (_, ms) = t.group("team", &["bob"], GroupMemberRole::Member);
    let m = messages(&fails(&mut t, ms[0].1, DecryptError::Other, 1));
    assert_eq!(
        (m[0].author.clone(), m[0].body.clone()),
        (
            MessageAuthor::Member("bob".into()),
            shown(DecryptError::Other, 1)
        )
    );
}

#[test]
fn a_blocked_friend_leaves_no_decryption_or_integrity_notice() {
    let mut t = T::started();
    let (bob, conn) = t.friend("bob");
    t.req(Request::SetBlocked {
        friend: bob,
        on: true,
    });
    assert!(messages(&fails(&mut t, conn, DecryptError::Header, 1)).is_empty());
    let skipped = crate::agent::MsgIntegrity::Error(crate::agent::MsgErrorType::BadHash);
    let ok = crate::chat::ChatMessage::new(crate::chat::ChatEvent::Ok).unwrap();
    let out = t.agent(AgentEvent::Msg {
        conn,
        msg_id: 1,
        body: crate::chat::encode_chat_message(&ok, false).unwrap(),
        integrity: skipped,
        broker_ts: 0,
    });
    assert!(messages(&out).is_empty());
}
