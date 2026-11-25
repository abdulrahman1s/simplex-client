//! Files: voice messages, sent with their length and refused where not allowed (Commands.hs `featureAllowed SCFVoice`; Internal.hs `prohibitedGroupContent`; ChatTests/Profiles.hs `testSetContactPrefs`).

use super::*;
use crate::types::model::{Allow, ChatPrefs};

fn voice(t: &mut T, chat: ChatRef, secs: u32) -> Vec<Event> {
    let path = put(t, "note.m4a");
    t.req(Request::SendFile {
        chat,
        path,
        kind: FileKind::Voice,
        preview: None,
        duration: Some(secs),
        text: String::new(),
    })
}

#[test]
fn a_voice_message_goes_with_its_length_and_shows_as_one() {
    let (mut t, fake) = setup();
    fake.with(|s| s.description = known());
    let (bob, _) = t.friend("bob");
    let out = voice(&mut t, ChatRef::Direct(bob), 12);
    let [m] = messages(&out).try_into().unwrap();
    let MessageBody::Voice(media) = &m.body else {
        panic!("{:?}", m.body)
    };
    assert_eq!(media.duration, Some(12));
    let ChatEvent::MsgNew(c) = sent_events(&t).remove(0) else {
        panic!()
    };
    assert!(matches!(c.content, MsgContent::Voice { duration: 12, .. }));
    assert!(c.file.is_some());
}

#[test]
fn a_voice_message_is_refused_when_the_friend_does_not_allow_it() {
    let (mut t, _) = setup();
    let (bob, conn) = t.friend("bob");
    let no = crate::chat::preferences::FeatureAllowed::No;
    t.peer_says(
        &conn,
        1,
        super::super::profile::their_info(None, |p| {
            p.voice = super::super::profile::pref(no);
        }),
    );
    let out = voice(&mut t, ChatRef::Direct(bob), 5);
    assert!(failed(&out, RequestKind::SendFile));
    assert!(
        sent_events(&t)
            .iter()
            .all(|e| !matches!(e, ChatEvent::MsgNew(_)))
    );
    // "always" from the user does not let the user's own through (testSetContactPrefs).
    t.req(Request::SetPrefs {
        friend: Some(bob),
        prefs: ChatPrefs {
            voice: Some(Allow::Always),
            ..ChatPrefs::default()
        },
    });
    assert!(failed(
        &voice(&mut t, ChatRef::Direct(bob), 5),
        RequestKind::SendFile
    ));
}

#[test]
fn a_received_voice_message_shows_with_its_length() {
    let (mut t, _) = setup();
    let (_, conn) = t.friend("bob");
    let content = MsgContent::Voice {
        text: String::new(),
        duration: 7,
    };
    let m = peer_offers(&mut t, &conn, offered(content, "voice.m4a", 3 * KB));
    let MessageBody::Voice(media) = &m.body else {
        panic!("{:?}", m.body)
    };
    assert_eq!(
        (media.duration, media.name.as_str()),
        (Some(7), "voice.m4a")
    );
}
