//! An owner's changes to the group itself, and moderation (Commands.hs `APIDeleteChat`, `APIUpdateGroupProfile`, `APIDeleteMemberChatItem`; ChatTests/Groups.hs `testGroupDelete`, `testUpdateGroupProfile`, `testGroupModerate`).

use crate::types::model::{
    ChatNotice, ChatRef, GroupPref, GroupPrefs, MemberRole, MessageAuthor, MessageBody,
};
use crate::types::{Request, RequestKind};

use super::manage::notices;
use super::messages::{failed, sent_to};
use crate::chat::ChatEvent;
use crate::chat::group::GroupMemberRole;
use crate::chat::group_prefs::FeatureEnabled;
use crate::engine::records::ItemRecord;
use crate::engine::tests::T;

fn said_by(t: &mut T, g: i64, member: i64, text: &str) -> i64 {
    let name = t.e.member(g, member).unwrap().profile.display_name;
    let mut it = ItemRecord::new(
        MessageAuthor::Member(name),
        MessageBody::Text(text.into()),
        0,
    );
    it.member = Some(member);
    it.shared_id = Some(text.as_bytes().to_vec());
    t.e.add_item(ChatRef::Group(g), &it).unwrap().id
}

#[test]
fn an_owner_deletes_the_group_for_everyone_and_nothing_of_it_is_left() {
    let mut t = T::started();
    let (g, m) = t.group("team", &["bob"], GroupMemberRole::Owner);
    said_by(&mut t, g, m[0].0, "hi");
    t.req(Request::DeleteGroup(g));
    assert!(matches!(
        sent_to(&t, &m[0].1).as_slice(),
        [ChatEvent::GrpDel]
    ));
    assert!(!t.e.state.groups.contains_key(&g));
    assert!(t.e.state.members.keys().all(|(group, _)| *group != g));
    let items = t.e.store.page(
        &crate::engine::records::keys::chat(ChatRef::Group(g)),
        None,
        5,
    );
    assert!(items.unwrap().is_empty());
    let left = |t: &mut T, p: String| t.e.store.keys_with_prefix("chat", p.as_bytes()).unwrap();
    assert!(left(&mut t, format!("member/{g}/")).is_empty());
    assert!(left(&mut t, format!("group/{g}")).is_empty());
}

#[test]
fn a_member_deletes_a_group_only_once_out_of_it_and_tells_nobody() {
    let mut t = T::started();
    let (g, m) = t.group("team", &["bob"], GroupMemberRole::Member);
    assert!(failed(
        &t.req(Request::DeleteGroup(g)),
        RequestKind::GroupAdmin
    ));
    t.req(Request::LeaveGroup(g));
    let sent = t.e.agent.sent().len();
    t.req(Request::DeleteGroup(g));
    assert_eq!(t.e.agent.sent().len(), sent);
    assert!(!t.e.state.groups.contains_key(&g));
    let _ = m;
}

#[test]
fn an_owner_renames_and_describes_the_group() {
    let mut t = T::started();
    let (g, m) = t.group("team", &["bob"], GroupMemberRole::Owner);
    let out = t.req(Request::SetGroupProfile {
        group: g,
        name: "crew".into(),
        description: Some("Welcome!".into()),
    });
    let told = sent_to(&t, &m[0].1);
    let [ChatEvent::GrpInfo(p)] = told.as_slice() else {
        panic!("{told:?}")
    };
    assert_eq!(
        (p.display_name.as_str(), p.description.as_deref()),
        ("crew", Some("Welcome!"))
    );
    assert_eq!(
        notices(&out),
        vec![ChatNotice::GroupNameChanged("crew".into())]
    );
    let bad = t.req(Request::SetGroupProfile {
        group: g,
        name: "#crew".into(),
        description: None,
    });
    assert!(
        bad.iter()
            .any(|e| matches!(e, crate::types::Event::Failed { .. }))
    );
}

#[test]
fn an_owner_turns_a_feature_off_and_the_rest_stay() {
    let mut t = T::started();
    let (g, m) = t.group("team", &["bob"], GroupMemberRole::Owner);
    let before = t.e.group(g).unwrap().profile.group_preferences;
    let off = GroupPrefs {
        voice: Some(GroupPref {
            on: false,
            role: None,
        }),
        files: Some(GroupPref {
            on: true,
            role: Some(MemberRole::Moderator),
        }),
        ..GroupPrefs::default()
    };
    t.req(Request::SetGroupPrefs {
        group: g,
        prefs: off.clone(),
    });
    let after = t.e.group(g).unwrap().profile.group_preferences.unwrap();
    assert_eq!(after.voice.as_ref().unwrap().enable, FeatureEnabled::Off);
    assert_eq!(
        after.files.as_ref().unwrap().role.as_deref(),
        Some("moderator")
    );
    assert_eq!(after.reactions, before.and_then(|b| b.reactions));
    assert_eq!(sent_to(&t, &m[0].1).len(), 1);
    t.req(Request::SetGroupPrefs {
        group: g,
        prefs: off,
    });
    assert_eq!(sent_to(&t, &m[0].1).len(), 1, "no change, nothing sent");
}

#[test]
fn a_moderator_deletes_a_members_message_for_everyone_but_not_a_higher_ones() {
    let mut t = T::started();
    let (g, m) = t.group("team", &["bob", "carol"], GroupMemberRole::Moderator);
    let spam = said_by(&mut t, g, m[1].0, "spam");
    t.req(Request::Moderate {
        group: g,
        item: spam,
    });
    let carol_id = t.e.member(g, m[1].0).unwrap().member_id;
    let told = sent_to(&t, &m[0].1);
    assert!(
        matches!(told.as_slice(), [ChatEvent::MsgDel { member_id: Some(id), .. }] if *id == carol_id),
        "{told:?}"
    );
    let chat = ChatRef::Group(g);
    let gone = t.e.item(chat, spam).unwrap();
    assert!(gone.is_none_or(|i| i.body == MessageBody::Deleted));
    let mut bob = t.e.member(g, m[0].0).unwrap();
    bob.role = GroupMemberRole::Admin;
    t.e.save_member(bob).unwrap();
    let theirs = said_by(&mut t, g, m[0].0, "mine");
    let out = t.req(Request::Moderate {
        group: g,
        item: theirs,
    });
    assert!(failed(&out, RequestKind::GroupAdmin));
}
