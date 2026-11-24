//! What a group member's role lets them do, and the group's other limits (Commands.hs `assertUserGroupRole` call sites; Internal.hs `sendHistory`).

use crate::chat::group::GroupMemberRole;

/// What a short link's user data holds at most, the profile shown before connecting: 13 784
/// bytes (Agent/Protocol.hs `userDataPaddedLength`).
pub use crate::agent::USER_DATA_PADDED_LENGTH as MAX_LINK_DATA;

/// Sending, replying, editing and files in a group need at least `author` (Commands.hs
/// `APISendMessages`, `APIUpdateChatItem`).
pub const ROLE_TO_SEND: GroupMemberRole = GroupMemberRole::Author;

/// Deleting one's own message for everyone needs at least `observer` (Commands.hs
/// `APIDeleteChatItem`).
pub const ROLE_TO_DELETE_OWN: GroupMemberRole = GroupMemberRole::Observer;

/// Deleting or blocking someone else's messages needs at least `moderator`, and at least the
/// role of the member (Commands.hs `APIDeleteMemberChatItem`, `APIBlockMembersForAll`).
pub const ROLE_TO_MODERATE: GroupMemberRole = GroupMemberRole::Moderator;

/// Blocking a member for everyone: as [`ROLE_TO_MODERATE`].
pub const ROLE_TO_BLOCK_FOR_ALL: GroupMemberRole = GroupMemberRole::Moderator;

/// Adding a member needs at least `admin`, and at least the role given (Commands.hs `APIAddMember`).
pub const ROLE_TO_ADD: GroupMemberRole = GroupMemberRole::Admin;

/// Removing a member needs at least `admin`, and at least the member's role (Commands.hs
/// `APIRemoveMembers`).
pub const ROLE_TO_REMOVE: GroupMemberRole = GroupMemberRole::Admin;

/// Making a group link needs at least `admin` (Commands.hs `APICreateGroupLink`).
pub const ROLE_TO_LINK: GroupMemberRole = GroupMemberRole::Admin;

/// Changing the group's name, picture or preferences needs `owner` (Commands.hs
/// `runUpdateGroupProfile`).
pub const ROLE_TO_EDIT_GROUP: GroupMemberRole = GroupMemberRole::Owner;

/// Messages a new member is sent from the group's history: 100 (Internal.hs `sendHistory`).
pub const HISTORY_ITEMS: usize = 100;
