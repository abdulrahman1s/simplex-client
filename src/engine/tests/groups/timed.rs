//! Disappearing messages in a group (ChatTests/Profiles.hs `testEnableTimedMessagesGroup`; Groups.hs `testGroupHistoryDisappearingMessage`).

use crate::types::model::{ChatNotice, ChatRef, MessageBody};
use crate::types::{Event, Request, RequestKind};

use super::messages::{failed, says, sent_to};
use super::seed::profile;
use crate::chat::ChatEvent;
use crate::chat::container::MsgContainer;
use crate::chat::group::GroupMemberRole;
use crate::chat::group_prefs::{FeatureEnabled, GroupFeature, GroupPreferences};
use crate::engine::tests::timed::{bodies, later, send};
use crate::engine::tests::{T, messages};

fn set_timer(t: &mut T, group: i64, ttl: Option<i64>) -> Vec<Event> {
    t.req(Request::SetTimer {
        chat: ChatRef::Group(group),
        ttl,
    })
}

fn timed(text: &str, ttl: i64) -> MsgContainer {
    let mut c = MsgContainer::text(text);
    c.ttl = Some(ttl);
    c
}

fn sent_feature(t: &T, conn: &crate::engine::agent_port::ConnId) -> Option<GroupFeature> {
    let mut infos = sent_to(t, conn).into_iter().filter_map(|e| match e {
        ChatEvent::GrpInfo(p) => p.group_preferences?.timed_messages,
        _ => None,
    });
    infos.next_back()
}

#[test]
fn an_owners_group_timer_goes_in_its_profile_and_on_every_message() {
    let mut t = T::started();
    let (g, members) = t.group("team", &["bob"], GroupMemberRole::Owner);
    let chat = ChatRef::Group(g);
    set_timer(&mut t, g, Some(3));
    let on = sent_feature(&t, &members[0].1).unwrap();
    assert_eq!((on.enable, on.ttl), (FeatureEnabled::On, Some(3)));

    let mine = send(&mut t, chat, "hi");
    assert_eq!(mine.timed.unwrap().ttl, 3);
    let last = sent_to(&t, &members[0].1).pop();
    assert!(matches!(last, Some(ChatEvent::MsgNew(c)) if c.ttl == Some(3)));
    let gone = messages(&later(&mut t, 4));
    assert_eq!(gone[0].body, MessageBody::Deleted);

    set_timer(&mut t, g, None);
    let off = sent_feature(&t, &members[0].1).unwrap();
    assert_eq!(
        (off.enable, off.ttl),
        (FeatureEnabled::Off, Some(86_400)),
        "SetGroupTimedMessages off keeps a day"
    );
    assert_eq!(send(&mut t, chat, "stays").timed, None);
    let notices: Vec<_> = (bodies(&mut t, chat).into_iter())
        .filter(|b| matches!(b, MessageBody::Notice(ChatNotice::Timer(_))))
        .collect();
    assert_eq!(notices.len(), 2, "{notices:?}");
}

#[test]
fn only_an_owner_sets_the_groups_timer() {
    let mut t = T::started();
    let (g, _) = t.group("team", &["bob"], GroupMemberRole::Admin);
    let out = set_timer(&mut t, g, Some(3));
    assert!(failed(&out, RequestKind::Timer));
}

#[test]
fn a_members_ttl_counts_only_while_the_group_has_timed_messages_on() {
    let mut t = T::started();
    let (g, members) = t.group("team", &["bob"], GroupMemberRole::Member);
    let host = members[0].1;
    let off = messages(&says(&mut t, &host, 1, timed("early", 3)));
    assert_eq!(off[0].timed, None);

    let mut p = profile("team");
    let mut feature = GroupFeature::new(FeatureEnabled::On);
    feature.ttl = Some(3);
    p.group_preferences = Some(GroupPreferences {
        timed_messages: Some(feature),
        ..GroupPreferences::default()
    });
    let out = t.peer_says(&host, 2, ChatEvent::GrpInfo(p));
    let notice = messages(&out).pop().unwrap();
    assert_eq!(notice.body, MessageBody::Notice(ChatNotice::Timer(Some(3))));

    let on = messages(&says(&mut t, &host, 3, timed("late", 3)));
    assert_eq!(on[0].timed.unwrap().delete_at, None, "counts once read");
    t.req(Request::MarkRead(ChatRef::Group(g)));
    later(&mut t, 4);
    assert!(t.e.item(ChatRef::Group(g), on[0].id).unwrap().is_none());
    assert!(t.e.item(ChatRef::Group(g), off[0].id).unwrap().is_some());
}
