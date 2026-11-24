//! The official client's limits in a group: roles, mentions and the picture, each refused with its own code.

use crate::types::model::{ChatRef, Picture};
use crate::types::{ErrorCode, FileKind, Request, RequestKind};

use super::messages::send;
use crate::chat::group::GroupMemberRole;
use crate::chat::group_prefs::{FeatureEnabled, GroupFeature, GroupPreferences};
use crate::engine::tests::T;
use crate::engine::tests::limits::code;
use crate::limits::MAX_PICTURE_URI;

fn jpeg(len: usize) -> Picture {
    let mut bytes = b"\xff\xd8\xff\xe0".to_vec();
    bytes.resize(len, 7);
    Picture {
        bytes,
        mime: "image/jpeg".into(),
    }
}

#[test]
fn an_observers_message_is_refused_as_not_allowed() {
    let mut t = T::started();
    let (g, _) = t.group("club", &["bob"], GroupMemberRole::Observer);
    let sends = t.e.agent.sent().len();
    let out = t.req(send(g, "hi", Vec::new()));
    assert_eq!(code(&out, RequestKind::Send), Some(ErrorCode::NotAllowed));
    assert_eq!(t.e.agent.sent().len(), sends);
}

#[test]
fn a_fourth_mention_is_refused_as_too_many_mentions() {
    let mut t = T::started();
    let names = ["ann", "bob", "cat", "dan"];
    let (g, members) = t.group("club", &names, GroupMemberRole::Owner);
    let text = "@ann @bob @cat @dan";
    let all: Vec<(String, i64)> = names
        .iter()
        .zip(&members)
        .map(|(n, (m, _))| ((*n).to_owned(), *m))
        .collect();
    let out = t.req(send(g, text, all.clone()));
    assert_eq!(
        code(&out, RequestKind::Send),
        Some(ErrorCode::TooManyMentions)
    );
    let out = t.req(send(g, text, all[..3].to_vec()));
    assert_eq!(code(&out, RequestKind::Send), None);
}

#[test]
fn a_member_adding_a_friend_is_refused_as_not_allowed() {
    let mut t = T::started();
    let (friend, _) = t.friend("eve");
    let (g, _) = t.group("club", &["bob"], GroupMemberRole::Member);
    let out = t.req(Request::AddMember { group: g, friend });
    assert_eq!(code(&out, RequestKind::Group), Some(ErrorCode::NotAllowed));
}

#[test]
fn only_the_owner_changes_the_group_picture_and_never_past_the_limit() {
    let mut t = T::started();
    let (g, _) = t.group("club", &["bob"], GroupMemberRole::Admin);
    let set = |group, len| Request::SetGroupPicture {
        group,
        picture: Some(jpeg(len)),
    };
    let out = t.req(set(g, 64));
    assert_eq!(code(&out, RequestKind::Group), Some(ErrorCode::NotAllowed));
    let (mine, _) = t.group("mine", &["bob"], GroupMemberRole::Owner);
    let out = t.req(set(mine, MAX_PICTURE_URI));
    assert_eq!(
        code(&out, RequestKind::Group),
        Some(ErrorCode::PictureTooLarge)
    );
    assert_eq!(code(&t.req(set(mine, 64)), RequestKind::Group), None);
}

#[test]
fn files_the_group_turned_off_are_refused_as_not_allowed() {
    let mut t = T::started();
    let (g, _) = t.group("club", &["bob"], GroupMemberRole::Member);
    let mut group = t.e.state.groups[&g].clone();
    let mut prefs = GroupPreferences::default_full();
    prefs.files = Some(GroupFeature::new(FeatureEnabled::Off));
    group.profile.group_preferences = Some(prefs);
    t.e.save_group(group).unwrap();
    let path = t.dir.0.join("files").join("a.pdf");
    std::fs::write(&path, b"pdf").unwrap();
    let out = t.req(Request::SendFile {
        chat: ChatRef::Group(g),
        path,
        kind: FileKind::File,
        preview: None,
        duration: None,
        text: String::new(),
    });
    assert_eq!(
        code(&out, RequestKind::SendFile),
        Some(ErrorCode::NotAllowed)
    );
}
