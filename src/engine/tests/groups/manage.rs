//! An admin's own changes to a group (Commands.hs `APIMembersRole`, `APIBlockMembersForAll`, `APIRemoveMembers`; ChatTests/Groups.hs `testUpdateMemberRole`, `testBlockForAllMembers`, `testGroupRemoveMember`).

use crate::types::model::{ChatNotice, MemberRole, MessageBody};
use crate::types::{Event, Request, RequestKind};

use super::messages::{failed, sent_to};
use crate::chat::ChatEvent;
use crate::chat::group::{GroupMemberRole, MemberRestrictionStatus};
use crate::engine::agent_port::ConnId;
use crate::engine::records::{ItemRecord, MemberStatus};
use crate::engine::tests::{Call, T, messages};

pub(super) fn notices(out: &[Event]) -> Vec<ChatNotice> {
    let all = messages(out).into_iter();
    all.filter_map(|m| match m.body {
        MessageBody::Notice(n) => Some(n),
        _ => None,
    })
    .collect()
}

/// The user has `role`; carol is a member, and bob is too, or the owner who invited the user
/// when the user is not the owner.
fn team(role: GroupMemberRole) -> (T, i64, (i64, ConnId), (i64, ConnId)) {
    let mut t = T::started();
    let (g, m) = t.group("team", &["bob", "carol"], role);
    (t, g, m[0], m[1])
}

#[test]
fn an_admin_changes_a_role_and_every_current_member_is_told() {
    let (mut t, g, bob, carol) = team(GroupMemberRole::Owner);
    let out = t.req(Request::SetMemberRole {
        group: g,
        member: carol.0,
        role: MemberRole::Admin,
    });
    let carol_id = t.e.member(g, carol.0).unwrap().member_id;
    for conn in [bob.1, carol.1] {
        let told = sent_to(&t, &conn);
        assert!(
            matches!(told.as_slice(), [ChatEvent::GrpMemRole { member_id, role: GroupMemberRole::Admin, .. }] if *member_id == carol_id),
            "{told:?}"
        );
    }
    assert_eq!(t.e.member(g, carol.0).unwrap().role, GroupMemberRole::Admin);
    let role = MemberRole::Admin;
    let who = "carol".to_owned();
    assert_eq!(
        notices(&out),
        vec![ChatNotice::MemberRole {
            by: None,
            who,
            role
        }]
    );
}

#[test]
fn a_role_change_needs_admin_and_both_roles() {
    let (mut t, g, _, carol) = team(GroupMemberRole::Admin);
    let to_owner = Request::SetMemberRole {
        group: g,
        member: carol.0,
        role: MemberRole::Owner,
    };
    assert!(
        failed(&t.req(to_owner), RequestKind::GroupAdmin),
        "an owner's role needs an owner"
    );
    let (mut t, g, _, carol) = team(GroupMemberRole::Moderator);
    let to_observer = Request::SetMemberRole {
        group: g,
        member: carol.0,
        role: MemberRole::Observer,
    };
    assert!(
        failed(&t.req(to_observer), RequestKind::GroupAdmin),
        "the admin floor"
    );
    assert!(t.e.agent.sent().is_empty());
}

#[test]
fn blocking_for_all_tells_the_others_and_not_the_member() {
    let (mut t, g, bob, carol) = team(GroupMemberRole::Moderator);
    let out = t.req(Request::BlockMemberForAll {
        group: g,
        member: carol.0,
        on: true,
    });
    assert!(sent_to(&t, &carol.1).is_empty());
    let told = sent_to(&t, &bob.1);
    assert!(
        matches!(told.as_slice(), [ChatEvent::GrpMemRestrict { restrictions, .. }] if restrictions.restriction == MemberRestrictionStatus::Blocked),
        "{told:?}"
    );
    assert!(t.e.member(g, carol.0).unwrap().blocked_by_admin);
    assert!(notices(&out).contains(&ChatNotice::MemberBlocked {
        by: None,
        who: "carol".into(),
        blocked: true
    }));
}

#[test]
fn removing_a_member_tells_everyone_then_lets_the_connection_go() {
    let (mut t, g, bob, carol) = team(GroupMemberRole::Admin);
    let mut said = ItemRecord::new(
        crate::types::model::MessageAuthor::Member("carol".into()),
        MessageBody::Text("spam".into()),
        0,
    );
    said.member = Some(carol.0);
    let spam =
        t.e.add_item(crate::types::model::ChatRef::Group(g), &said)
            .unwrap();
    let out = t.req(Request::RemoveMember {
        group: g,
        member: carol.0,
        messages: true,
    });
    for conn in [bob.1, carol.1] {
        let told = sent_to(&t, &conn);
        assert!(
            matches!(
                told.as_slice(),
                [ChatEvent::GrpMemDel { messages: true, .. }]
            ),
            "{told:?}"
        );
    }
    let m = t.e.member(g, carol.0).unwrap();
    assert_eq!((m.status, m.conn), (MemberStatus::Removed, None));
    let deleted =
        t.e.agent
            .calls
            .iter()
            .any(|c| matches!(c, Call::Delete { conn } if *conn == carol.1));
    assert!(deleted, "after the removal, once it is delivered");
    let chat = crate::types::model::ChatRef::Group(g);
    assert_eq!(
        t.e.item(chat, spam.id).unwrap().unwrap().body,
        MessageBody::Deleted
    );
    assert!(notices(&out).contains(&ChatNotice::MemberRemoved {
        by: None,
        who: "carol".into()
    }));
}

#[test]
fn removing_needs_admin_and_the_members_role() {
    let (mut t, g, bob, _) = team(GroupMemberRole::Moderator);
    let out = t.req(Request::RemoveMember {
        group: g,
        member: bob.0,
        messages: false,
    });
    assert!(failed(&out, RequestKind::GroupAdmin));
    assert_eq!(
        t.e.member(g, bob.0).unwrap().status,
        MemberStatus::Connected
    );
}
