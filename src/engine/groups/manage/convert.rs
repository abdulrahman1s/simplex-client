//! Roles between the page's `MemberRole` and the protocol's `GroupMemberRole`.

use crate::types::model::MemberRole;

use crate::chat::group::GroupMemberRole;

#[must_use]
pub const fn to_chat(r: MemberRole) -> GroupMemberRole {
    match r {
        MemberRole::Observer => GroupMemberRole::Observer,
        MemberRole::Author => GroupMemberRole::Author,
        MemberRole::Member => GroupMemberRole::Member,
        MemberRole::Moderator => GroupMemberRole::Moderator,
        MemberRole::Admin => GroupMemberRole::Admin,
        MemberRole::Owner => GroupMemberRole::Owner,
    }
}

/// A relay's role, or one this client does not know, shows as the lowest.
#[must_use]
pub const fn to_wire(r: &GroupMemberRole) -> MemberRole {
    match r {
        GroupMemberRole::Author => MemberRole::Author,
        GroupMemberRole::Member => MemberRole::Member,
        GroupMemberRole::Moderator => MemberRole::Moderator,
        GroupMemberRole::Admin => MemberRole::Admin,
        GroupMemberRole::Owner => MemberRole::Owner,
        GroupMemberRole::Observer | GroupMemberRole::Relay | GroupMemberRole::Unknown(_) => {
            MemberRole::Observer
        }
    }
}
