//! Disappearing messages with a friend (ChatTests/Profiles.hs `testEnableTimedMessagesContact`; Direct.hs `testDirectTimedMessage`).

use std::time::Duration;

use crate::types::model::{ChatMessage, ChatNotice, ChatRef, MessageBody, Timed};
use crate::types::{Event, Request};

use super::files::sent_events;
use super::{T, messages};
use crate::chat::ChatEvent;
use crate::chat::container::MsgContainer;
use crate::chat::preferences::{FeatureAllowed, TimedPref};
use crate::engine::agent_port::ConnId;

pub(super) fn set_timer(t: &mut T, friend: i64, ttl: Option<i64>) -> Vec<Event> {
    t.req(Request::SetTimer {
        chat: ChatRef::Direct(friend),
        ttl,
    })
}

pub(super) fn send(t: &mut T, chat: ChatRef, text: &str) -> ChatMessage {
    let out = t.req(Request::Send {
        chat,
        text: text.into(),
        reply_to: None,
        mentions: Vec::new(),
    });
    messages(&out).pop().unwrap()
}

/// A message from the friend that lasts `ttl` seconds.
pub(super) fn timed_msg(text: &str, ttl: Option<i64>) -> ChatEvent {
    let mut c = MsgContainer::text(text);
    c.ttl = ttl;
    ChatEvent::MsgNew(c)
}

/// `secs` later, after the engine's timers ran.
pub(super) fn later(t: &mut T, secs: u64) -> Vec<Event> {
    t.clock.advance(Duration::from_secs(secs));
    t.e.tick();
    t.out()
}

/// The conversation's bodies, oldest first.
pub(super) fn bodies(t: &mut T, chat: ChatRef) -> Vec<MessageBody> {
    let items = t.e.items_page(chat, None, 100).unwrap();
    items.into_iter().map(|m| m.body).collect()
}

/// The timer in the last profile the user sent.
pub(super) fn sent_timer(t: &T) -> Option<TimedPref> {
    let mut infos = sent_events(t).into_iter().filter_map(|e| match e {
        ChatEvent::Info { profile, .. } => Some(profile),
        _ => None,
    });
    infos.next_back()?.preferences?.timed_messages
}

/// The timer the user keeps for `friend`.
pub(super) fn my_ttl(t: &T, friend: i64) -> Option<i64> {
    let mine = t.e.state.friends[&friend].my_prefs.as_ref()?;
    mine.timed_messages.as_ref()?.ttl
}

pub(super) fn ttl_sent(t: &T) -> Option<i64> {
    let mut news = sent_events(t).into_iter().filter_map(|e| match e {
        ChatEvent::MsgNew(c) => Some(c.ttl),
        _ => None,
    });
    news.next_back().flatten()
}

pub(super) fn friend(t: &mut T) -> (i64, ConnId) {
    t.friend("bob")
}

#[test]
fn a_timer_set_for_a_friend_is_offered_and_both_sides_messages_go_after_it() {
    let mut t = T::started();
    let (bob, conn) = friend(&mut t);
    let chat = ChatRef::Direct(bob);
    set_timer(&mut t, bob, Some(3));
    let offered = TimedPref {
        allow: FeatureAllowed::Yes,
        ttl: Some(3),
    };
    assert_eq!(sent_timer(&t), Some(offered));
    assert_eq!(my_ttl(&t, bob), Some(3));

    let mine = send(&mut t, chat, "hi");
    assert_eq!(ttl_sent(&t), Some(3), "the message carries its ttl");
    let now = crate::engine::clock::Clock::wall_ms(&t.clock);
    let at = Some(now + 3000);
    assert_eq!(
        mine.timed,
        Some(Timed {
            ttl: 3,
            delete_at: at
        })
    );

    let theirs = messages(&t.peer_says(&conn, 1, timed_msg("hey", Some(3))))[0].clone();
    assert_eq!(
        theirs.timed,
        Some(Timed {
            ttl: 3,
            delete_at: None
        })
    );

    let gone = messages(&later(&mut t, 4));
    assert_eq!(
        gone.len(),
        1,
        "only the user's: theirs counts from being read"
    );
    assert_eq!(
        (gone[0].id, &gone[0].body),
        (mine.id, &MessageBody::Deleted)
    );
    assert!(
        t.e.item(chat, mine.id).unwrap().is_none(),
        "deleted for good"
    );

    later(&mut t, 60);
    assert!(
        t.e.item(chat, theirs.id).unwrap().is_some(),
        "unread, it stays"
    );
    let read = messages(&t.req(Request::MarkRead(chat)));
    let started = crate::engine::clock::Clock::wall_ms(&t.clock) + 3000;
    assert_eq!(read[0].timed.unwrap().delete_at, Some(started));
    later(&mut t, 4);
    assert_eq!(
        bodies(&mut t, chat),
        vec![
            MessageBody::Notice(ChatNotice::Connected),
            MessageBody::Notice(ChatNotice::Timer(Some(3)))
        ],
        "only the notices are left"
    );
}
