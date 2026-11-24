//! Someone in a group, as the group's list or a message names them.

use super::{Friend, Group};
use crate::client::Client;
use crate::types::model::GroupMember;

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
}
