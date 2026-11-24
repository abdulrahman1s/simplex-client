//! A repeated `x.grp.inv` changes the group only before the user joins, and only from the friend who invited (stricter than 7.1.4).

use crate::types::Request;

use super::members_join::{invitation, invited};
use crate::chat::ChatEvent;
use crate::chat::group::GroupMemberRole;
use crate::engine::tests::T;

#[test]
fn a_repeated_invitation_after_joining_changes_nothing() {
    let mut t = T::started();
    let (g, ann) = invited(&mut t);
    t.req(Request::JoinGroup(g));
    let before = t.e.state.groups[&g].clone();
    let again = invitation(1, GroupMemberRole::Owner, GroupMemberRole::Observer);
    t.peer_says(&ann, 2, ChatEvent::GrpInv(again));
    let after = &t.e.state.groups[&g];
    assert_eq!(
        (&after.my_role, &after.my_member_id),
        (&before.my_role, &before.my_member_id)
    );
}

#[test]
fn a_repeated_invitation_from_another_friend_changes_nothing() {
    let mut t = T::started();
    let (g, _) = invited(&mut t);
    let (_, eve) = t.friend("eve");
    let again = invitation(1, GroupMemberRole::Owner, GroupMemberRole::Admin);
    t.peer_says(&eve, 1, ChatEvent::GrpInv(again));
    assert_eq!(t.e.state.groups[&g].my_role, GroupMemberRole::Member);
}
