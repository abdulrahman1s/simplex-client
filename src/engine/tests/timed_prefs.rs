//! A friend's side of the timer: turned off, and theirs taken over (ChatTests/Profiles.hs `testEnableTimedMessagesContact`).

use crate::types::model::{ChatNotice, ChatRef, MessageBody};

use super::profile::their_info;
use super::timed::{bodies, friend, send, sent_timer, set_timer, timed_msg, ttl_sent};
use super::{T, messages};
use crate::chat::preferences::{FeatureAllowed, TimedPref};

#[test]
fn turning_the_timer_off_turns_it_off_for_both_and_keeps_the_ttl() {
    let mut t = T::started();
    let (bob, conn) = friend(&mut t);
    let chat = ChatRef::Direct(bob);
    set_timer(&mut t, bob, Some(3));
    set_timer(&mut t, bob, None);
    let off = TimedPref {
        allow: FeatureAllowed::No,
        ttl: Some(3),
    };
    assert_eq!(sent_timer(&t), Some(off), "TMEDisableKeepTTL");
    assert_eq!(send(&mut t, chat, "hi").timed, None);
    assert_eq!(ttl_sent(&t), None);
    let theirs = messages(&t.peer_says(&conn, 1, timed_msg("hey", Some(3))));
    assert_eq!(
        theirs[0].timed, None,
        "a ttl is not kept while the user said no"
    );
    let sent = t.e.agent.sent().len();
    set_timer(&mut t, bob, None);
    assert_eq!(t.e.agent.sent().len(), sent, "no change, nothing sent");
}

#[test]
fn their_timer_becomes_the_users_for_them_without_an_answer() {
    let mut t = T::started();
    let (bob, conn) = friend(&mut t);
    let chat = ChatRef::Direct(bob);
    let sent = t.e.agent.sent().len();
    let with = |ttl| {
        their_info(None, move |p| {
            p.timed_messages = Some(TimedPref {
                allow: FeatureAllowed::Yes,
                ttl,
            });
        })
    };
    t.peer_says(&conn, 1, with(Some(30)));
    assert_eq!(t.e.agent.sent().len(), sent, "adopted silently");
    assert_eq!(send(&mut t, chat, "hi").timed.unwrap().ttl, 30);
    t.peer_says(&conn, 2, with(None));
    assert_eq!(send(&mut t, chat, "again").timed, None);
    let notices: Vec<_> = (bodies(&mut t, chat).into_iter())
        .filter(|b| matches!(b, MessageBody::Notice(ChatNotice::Timer(_))))
        .collect();
    assert_eq!(
        notices,
        vec![
            MessageBody::Notice(ChatNotice::Timer(Some(30))),
            MessageBody::Notice(ChatNotice::Timer(None))
        ]
    );
}
