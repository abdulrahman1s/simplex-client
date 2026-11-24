//! Someone in a group, as the group's list or a message names them.

use super::{Friend, Group};
use crate::client::{Client, Error, Result};
use crate::types::model::{GroupMember, MemberRole};
use crate::types::{ErrorCode, Request, RequestKind};

#[derive(Clone, Debug)]
pub struct Member {
    client: Client,
    group: i64,
    name: String,
    /// From the group's member list; a message or a notice names the member only.
    info: Option<GroupMember>,
}

impl Member {
    pub(in crate::client) const fn named(client: Client, group: i64, name: String) -> Self {
        Self {
            client,
            group,
            name,
            info: None,
        }
    }

    pub(in crate::client) fn listed(client: Client, group: i64, info: GroupMember) -> Self {
        let name = info.name.clone();
        Self {
            client,
            group,
            name,
            info: Some(info),
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The member's number in the group, when it came from the member list.
    #[must_use]
    pub fn id(&self) -> Option<i64> {
        self.info.as_ref().map(|m| m.id)
    }

    #[must_use]
    pub fn group(&self) -> Group {
        self.client.group_or_id(self.group)
    }

    /// The member is also the user's friend.
    #[must_use]
    pub fn friend(&self) -> Option<Friend> {
        let id = self.info.as_ref()?.friend?;
        Some(self.client.friend_or_id(id))
    }

    #[must_use]
    pub const fn info(&self) -> Option<&GroupMember> {
        self.info.as_ref()
    }

    /// An admin changes the member's role; the group is told.
    pub fn set_role(&self, role: MemberRole) -> Result<()> {
        let member = self.listed_id()?;
        let group = self.group;
        self.client.request(Request::SetMemberRole {
            group,
            member,
            role,
        })
    }

    /// A moderator blocks or unblocks the member's messages for everyone.
    pub fn block_for_all(&self, on: bool) -> Result<()> {
        let member = self.listed_id()?;
        let group = self.group;
        self.client
            .request(Request::BlockMemberForAll { group, member, on })
    }

    /// An admin removes the member; with `messages` what they wrote shows deleted for all.
    pub fn remove(&self, messages: bool) -> Result<()> {
        let member = self.listed_id()?;
        let group = self.group;
        self.client.request(Request::RemoveMember {
            group,
            member,
            messages,
        })
    }

    /// Asks the member to talk directly, with `text` if given; they show as a friend,
    /// connecting, until they accept.
    pub fn contact(&self, text: Option<&str>) -> Result<()> {
        let member = self.listed_id()?;
        let group = self.group;
        self.client.request(Request::ContactMember {
            group,
            member,
            text: text.map(str::to_owned),
        })
    }

    /// Only a member from the group's list has a number to act on.
    fn listed_id(&self) -> Result<i64> {
        self.id().ok_or_else(|| Error::Failed {
            request: RequestKind::GroupAdmin,
            code: ErrorCode::RequestFailed,
            detail: "a member not from the group's list".into(),
        })
    }
}
