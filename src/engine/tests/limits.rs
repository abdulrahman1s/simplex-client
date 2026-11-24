//! The official client's limits in a direct chat: each refused with its own code, or ignored when received.

use std::time::Duration;

use crate::types::model::{ChatMessage, ChatProfile, ChatRef};
use crate::types::{ErrorCode, Event, Request, RequestKind};

use super::profile::{pref, their_info};
use super::{T, messages};
use crate::chat::ChatEvent;
use crate::chat::container::MsgContainer;
use crate::chat::ids::SharedMsgId;
use crate::chat::preferences::FeatureAllowed;
use crate::chat::reaction::MsgReaction;
use crate::limits::MAX_ENCODED_MSG_LENGTH;

/// The code `out` failed `kind` with.
pub(super) fn code(out: &[Event], kind: RequestKind) -> Option<ErrorCode> {
    out.iter().find_map(|e| match e {
        Event::Failed { request, code, .. } if *request == kind => Some(*code),
        _ => None,
    })
}

fn send(t: &mut T, bob: i64, text: &str) -> Vec<Event> {
    t.req(Request::Send {
        chat: ChatRef::Direct(bob),
        text: text.into(),
        reply_to: None,
        mentions: Vec::new(),
    })
}

fn react(t: &mut T, bob: i64, item: i64, emoji: &str) -> Vec<Event> {
    t.req(Request::React {
        chat: ChatRef::Direct(bob),
        item,
        emoji: emoji.into(),
        on: true,
    })
}

fn theirs(t: &mut T, conn: &crate::engine::agent_port::ConnId) -> ChatMessage {
    let said = ChatEvent::MsgNew(MsgContainer::text("gg"));
    messages(&t.peer_says(conn, 1, said)).remove(0)
}

#[test]
fn a_reaction_outside_the_official_set_is_refused() {
    let mut t = T::started();
    let (bob, conn) = t.friend("bob");
    let m = theirs(&mut t, &conn);
    let sends = t.e.agent.sent().len();
    let out = react(&mut t, bob, m.id, "🤡");
    assert_eq!(
        code(&out, RequestKind::React),
        Some(ErrorCode::ReactionNotAllowed)
    );
    assert_eq!(t.e.agent.sent().len(), sends, "nothing reaches the friend");
}

#[test]
fn a_fourth_reaction_of_the_user_is_refused_as_not_allowed() {
    let mut t = T::started();
    let (bob, conn) = t.friend("bob");
    let m = theirs(&mut t, &conn);
    for e in ["👍", "😀", "🚀"] {
        assert_eq!(code(&react(&mut t, bob, m.id, e), RequestKind::React), None);
    }
    let out = react(&mut t, bob, m.id, "✅");
    assert_eq!(
        code(&out, RequestKind::React),
        Some(ErrorCode::ReactionNotAllowed)
    );
}

#[test]
fn reactions_the_friend_turned_off_are_refused_as_not_allowed() {
    let mut t = T::started();
    let (bob, conn) = t.friend("bob");
    let m = theirs(&mut t, &conn);
    let off = their_info(None, |p| p.reactions = pref(FeatureAllowed::No));
    t.peer_says(&conn, 2, off);
    let out = react(&mut t, bob, m.id, "👍");
    assert_eq!(code(&out, RequestKind::React), Some(ErrorCode::NotAllowed));
}

#[test]
fn a_friends_fourth_reaction_is_ignored() {
    let mut t = T::started();
    let (bob, conn) = t.friend("bob");
    let m = messages(&send(&mut t, bob, "mine")).remove(0);
    let it = t.e.item(ChatRef::Direct(bob), m.id).unwrap().unwrap();
    let id = SharedMsgId(it.shared_id.unwrap());
    let react = |c| ChatEvent::MsgReact {
        msg_id: id.clone(),
        member_id: None,
        scope: None,
        reaction: MsgReaction::Emoji(c),
        add: true,
    };
    for (c, n) in ['👍', '😀', '🚀'].into_iter().zip(1..) {
        let out = t.peer_says(&conn, n, react(c));
        assert_eq!(
            messages(&out)[0].reactions.len(),
            usize::try_from(n).unwrap()
        );
    }
    let out = t.peer_says(&conn, 4, react('✅'));
    assert!(messages(&out).is_empty(), "{out:?}");
}

#[test]
fn a_message_over_the_encoded_limit_is_refused_even_when_it_would_compress() {
    let mut t = T::started();
    let (bob, _) = t.friend("bob");
    let sends = t.e.agent.sent().len();
    let text = "a".repeat(MAX_ENCODED_MSG_LENGTH - 10);
    let out = send(&mut t, bob, &text);
    assert_eq!(code(&out, RequestKind::Send), Some(ErrorCode::TextTooLong));
    assert!(messages(&out).is_empty());
    assert_eq!(t.e.agent.sent().len(), sends);
}

#[test]
fn an_edit_after_a_day_is_refused_as_not_allowed() {
    let mut t = T::started();
    let (bob, _) = t.friend("bob");
    let m = messages(&send(&mut t, bob, "old")).remove(0);
    t.clock.advance(Duration::from_secs(25 * 3600));
    let out = t.req(Request::Edit {
        chat: ChatRef::Direct(bob),
        item: m.id,
        text: "new".into(),
    });
    assert_eq!(code(&out, RequestKind::Edit), Some(ErrorCode::NotAllowed));
}

#[test]
fn a_changed_display_name_must_be_one_upstream_keeps() {
    let mut t = T::started();
    let named = |name: &str| {
        Request::SetProfile(ChatProfile {
            name: name.into(),
            ..ChatProfile::default()
        })
    };
    let out = t.req(named("J.Doe"));
    assert_eq!(
        code(&out, RequestKind::SetProfile),
        Some(ErrorCode::InvalidName)
    );
    let out = t.req(named("John Doe"));
    assert_eq!(code(&out, RequestKind::SetProfile), None);
    assert_eq!(t.e.state.user.name, "John Doe");
}
