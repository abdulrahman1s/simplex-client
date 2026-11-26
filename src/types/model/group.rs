//! Groups and their members.

use serde::{Deserialize, Serialize};

use super::{ChatRef, Picture, Playing, Presence};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Membership {
    Invited,
    Joining,
    #[default]
    Member,
    Left,
}

/// A member's role (Types/Shared.hs `GroupMemberRole`), lowest first; a relay's is not one a
/// member is given here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberRole {
    Observer,
    Author,
    #[default]
    Member,
    Moderator,
    Admin,
    Owner,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatGroup {
    pub id: i64,
    pub name: String,
    pub membership: Membership,
    pub members: Option<u32>,
    pub member_list: Vec<GroupMember>,
    pub muted: bool,
    pub voice: Vec<i64>,
    pub link: Option<String>,
    pub picture: Option<Picture>,
    pub owner: bool,
    /// The user's role in the group.
    #[serde(default)]
    pub role: MemberRole,
    /// The group's description, its welcome message.
    #[serde(default)]
    pub description: Option<String>,
    /// The user's own name for it, never sent; empty when none.
    #[serde(default)]
    pub alias: String,
    /// As `Friend::item_ttl`.
    #[serde(default)]
    pub item_ttl: Option<i64>,
    /// Seconds new messages last, when the group has disappearing messages on.
    #[serde(default)]
    pub timer: Option<i64>,
    /// A channel's part (`useRelays`); `None` for a classic group.
    #[serde(default)]
    pub channel: Option<super::ChannelInfo>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupMember {
    pub id: i64,
    pub name: String,
    pub friend: Option<i64>,
    pub me: bool,
    pub owner: bool,
    pub presence: Presence,
    pub playing: Option<Playing>,
    pub blocked: bool,
    pub picture: Option<Picture>,
    #[serde(default)]
    pub role: MemberRole,
    /// A moderator blocked them for everyone.
    #[serde(default)]
    pub blocked_for_all: bool,
}

impl ChatGroup {
    /// The group's conversation.
    #[must_use]
    pub const fn chat(&self) -> ChatRef {
        ChatRef::Group(self.id)
    }
}
