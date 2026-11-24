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
    /// Seconds new messages last, when the group has disappearing messages on.
    #[serde(default)]
    pub timer: Option<i64>,
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
}

impl ChatGroup {
    /// The group's conversation.
    #[must_use]
    pub const fn chat(&self) -> ChatRef {
        ChatRef::Group(self.id)
    }
}
