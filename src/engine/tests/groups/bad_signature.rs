//! A signed message whose signature fails, and changes to a message whose signature verified (Subscriber.hs `withVerifiedMsg`, `requireVerifiedEdit`, `requireVerifiedDelete`; `RGEMsgBadSignature`).

use crate::types::model::{ChatRef, MessageAuthor, MessageBody};

use super::super::T;
use super::forward::{forwarded, from_host, fwd, json};
use super::forward_signed::{keyed, rejected, signed, text};
use super::messages::{member_id, one};
use crate::chat::ChatEvent;
use crate::chat::container::MsgMentions;
use crate::chat::content::MsgContent;
use crate::chat::group::GroupMemberRole;
use crate::chat::ids::SharedMsgId;
use crate::crypto::{Alg, KeyPair};

/// 2026-11-01T10:00:00Z, the time `fwd` names.
const FORWARD_AT_MS: i64 = 1_793_527_200_000;

fn update(msg_id: SharedMsgId, text: &str) -> ChatEvent {
    ChatEvent::MsgUpdate {
        msg_id,
        content: MsgContent::Text { text: text.into() },
        mentions: MsgMentions::new(),
        ttl: None,
        live: None,
        scope: None,
        as_group: None,
    }
}

fn del(msg_id: SharedMsgId) -> ChatEvent {
    ChatEvent::MsgDel {
        msg_id,
        member_id: None,
        scope: None,
        only_history: false,
    }
}

#[test]
fn a_forwarded_message_with_a_bad_signature_is_rejected_from_its_author_at_the_forwards_time() {
    let mut t = T::started();
    let (g, ms) = t.group("team", &["bob", "carol"], GroupMemberRole::Member);
    let (host, carol) = (ms[0], ms[1]);
    keyed(&mut t, g, carol.0);
    let who = member_id(&t, g, carol.0);
    let other = KeyPair::generate(Alg::Ed25519).unwrap();
    let bad = forwarded(&fwd(who.clone(), "carol"), &signed(&other, &who, text("x")));
    let m = one(&from_host(&mut t, &host.1, 1, bad));
    assert_eq!(
        (m.body, m.author, m.at),
        (
            rejected(),
            MessageAuthor::Member("carol".into()),
            FORWARD_AT_MS
        )
    );
}

#[test]
fn an_unsigned_edit_or_deletion_of_a_verified_message_is_rejected_as_a_bad_signature() {
    let mut t = T::started();
    let (g, ms) = t.group("team", &["bob", "carol"], GroupMemberRole::Member);
    let (carol, conn) = ms[1];
    let kp = keyed(&mut t, g, carol);
    let who = member_id(&t, g, carol);
    let id = one(&from_host(
        &mut t,
        &conn,
        1,
        signed(&kp, &who, text("signed")),
    ))
    .id;
    let item = |t: &mut T| t.e.item(ChatRef::Group(g), id).unwrap().unwrap();
    let shared = SharedMsgId(item(&mut t).shared_id.unwrap());

    let forged_edit = json(update(shared.clone(), "forged"));
    let m = one(&from_host(&mut t, &conn, 2, forged_edit));
    assert_eq!(
        (m.body, m.author),
        (rejected(), MessageAuthor::Member("carol".into()))
    );
    let forged_del = json(del(shared.clone()));
    assert_eq!(
        one(&from_host(&mut t, &conn, 3, forged_del)).body,
        rejected()
    );
    let kept = item(&mut t);
    assert_eq!(
        (kept.body, kept.edited),
        (MessageBody::Text("signed".into()), false)
    );

    let edit = signed(&kp, &who, json(update(shared, "fixed")));
    from_host(&mut t, &conn, 4, edit);
    assert_eq!(item(&mut t).body, MessageBody::Text("fixed".into()));
}

#[test]
fn an_unsigned_edit_of_an_unsigned_message_is_taken() {
    let mut t = T::started();
    let (g, ms) = t.group("team", &["bob", "carol"], GroupMemberRole::Member);
    let (carol, conn) = ms[1];
    keyed(&mut t, g, carol);
    let id = one(&from_host(&mut t, &conn, 1, text("plain"))).id;
    let it = t.e.item(ChatRef::Group(g), id).unwrap().unwrap();
    assert!(!it.verified);
    let shared = SharedMsgId(it.shared_id.unwrap());
    from_host(&mut t, &conn, 2, json(update(shared, "edited")));
    let it = t.e.item(ChatRef::Group(g), id).unwrap().unwrap();
    assert_eq!(it.body, MessageBody::Text("edited".into()));
}
