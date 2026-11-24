//! SetPrefs: the user's own preferences and their overrides for one friend (ChatTests/Profiles.hs `testSetContactPrefs`, `testUpdateMultipleUserPrefs`).

use crate::types::model::{Allow, ChatFeature, ChatPrefs, ChatRef, MessageBody, PrefEnabled};
use crate::types::{Event, Request};

use super::T;
use super::profile::{pref, their_info};
use super::timed::{my_ttl, set_timer};
use crate::chat::ChatEvent;
use crate::chat::container::MsgContainer;
use crate::chat::content::MsgContent;
use crate::chat::preferences::{FeatureAllowed, Preferences};
use crate::engine::agent_port::ConnId;

fn set(t: &mut T, friend: Option<i64>, prefs: ChatPrefs) -> Vec<Event> {
    t.req(Request::SetPrefs { friend, prefs })
}

fn voice(a: Allow) -> ChatPrefs {
    ChatPrefs {
        voice: Some(a),
        ..ChatPrefs::default()
    }
}

/// The preferences in each profile sent to `conn`, in order.
fn sent_prefs(t: &T, conn: &ConnId) -> Vec<Preferences> {
    let sent = t.e.agent.sent();
    let to = sent.iter().filter(|(c, _)| c == conn);
    to.flat_map(|(_, b)| crate::chat::parse_chat_messages(b).unwrap())
        .filter_map(|m| match m.event {
            ChatEvent::Info { profile, .. } => profile.preferences,
            _ => None,
        })
        .collect()
}

fn voice_sent(t: &T, conn: &ConnId) -> Vec<FeatureAllowed> {
    let prefs = sent_prefs(t, conn).into_iter();
    prefs.map(|p| p.voice.unwrap().allow).collect()
}

#[test]
fn the_users_own_preference_goes_to_each_friend_it_changes_and_an_override_wins() {
    let mut t = T::started();
    let (bob, bob_conn) = t.friend("bob");
    let (_, carol_conn) = t.friend("carol");
    let out = set(&mut t, None, voice(Allow::No));
    let [Event::Profile(p)] = out.as_slice() else {
        panic!("{out:?}")
    };
    assert_eq!(p.prefs.voice, Some(Allow::No));
    set(&mut t, Some(bob), voice(Allow::Always));
    // Bob's override stands, so only carol's profile changes.
    set(&mut t, None, voice(Allow::Yes));
    use FeatureAllowed::{Always, No, Yes};
    assert_eq!(voice_sent(&t, &bob_conn), vec![No, Always]);
    assert_eq!(voice_sent(&t, &carol_conn), vec![No, Yes]);
}

#[test]
fn an_override_that_changes_nothing_sends_nothing() {
    let mut t = T::started();
    let (bob, _) = t.friend("bob");
    set(&mut t, Some(bob), ChatPrefs::default());
    // The default already: bob's profile is the same.
    set(&mut t, Some(bob), voice(Allow::Yes));
    assert!(t.e.agent.sent().is_empty(), "{:?}", t.e.agent.sent());
}

#[test]
fn always_against_their_no_is_on_for_their_messages_only() {
    let mut t = T::started();
    let (bob, conn) = t.friend("bob");
    t.peer_says(
        &conn,
        1,
        their_info(None, |p| p.voice = pref(FeatureAllowed::No)),
    );
    set(&mut t, Some(bob), voice(Allow::Always));
    let f = t.e.state.friends[&bob].clone();
    let on = t.e.enabled(&f, ChatFeature::Voice);
    assert_eq!(
        on,
        PrefEnabled {
            for_user: false,
            for_contact: true
        },
        "Voice messages: enabled for contact"
    );
    let shown = &t.req(Request::Refresh);
    let friend = shown.iter().find_map(|e| match e {
        Event::Chats { friends, .. } => friends.iter().find(|x| x.id == bob).cloned(),
        _ => None,
    });
    let friend = friend.unwrap();
    assert_eq!(friend.features[&ChatFeature::Voice], on);
    assert_eq!(friend.prefs.voice, Some(Allow::Always));
}

#[test]
fn a_voice_message_the_user_does_not_allow_is_not_kept() {
    let mut t = T::started();
    let (bob, conn) = t.friend("bob");
    set(&mut t, None, voice(Allow::No));
    let content = MsgContent::Voice {
        text: String::new(),
        duration: 10,
    };
    let out = t.peer_says(&conn, 1, ChatEvent::MsgNew(MsgContainer::simple(content)));
    assert!(super::messages(&out).is_empty(), "{out:?}");
    let items = t.e.items_page(ChatRef::Direct(bob), None, 10).unwrap();
    assert!(
        items
            .iter()
            .all(|m| matches!(m.body, MessageBody::Notice(_)))
    );
}

#[test]
fn preferences_keep_the_timer_and_last_across_a_restart() {
    let mut t = T::started();
    let (bob, _) = t.friend("bob");
    set_timer(&mut t, bob, Some(60));
    set(&mut t, Some(bob), voice(Allow::No));
    set(
        &mut t,
        None,
        ChatPrefs {
            calls: Some(Allow::No),
            ..ChatPrefs::default()
        },
    );
    assert_eq!(my_ttl(&t, bob), Some(60), "naming no timer keeps the timer");
    let t = t.restarted();
    let f = t.e.state.friends[&bob].clone();
    assert_eq!(my_ttl(&t, bob), Some(60));
    assert!(!t.e.enabled(&f, ChatFeature::Voice).for_contact);
    assert!(!t.e.enabled(&f, ChatFeature::Calls).for_user);
}
