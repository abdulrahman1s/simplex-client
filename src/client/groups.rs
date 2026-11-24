//! Groups: make one, invite friends, join, leave, list members.

use super::error::{Result, unexpected};
use super::handle::Client;
use super::waiters::{Answer, Want};
use crate::types::model::GroupMember;
use crate::types::{Event, Request, RequestKind};

impl Client {
    /// Makes a group with the user as its owner; `Event::GroupAdd` follows. `create_group`
    /// waits for it and returns the `Group`.
    pub fn new_group(&self, name: &str) -> Result<()> {
        self.request(Request::NewGroup(name.to_owned()))
    }

    /// Invites a friend into a group the user owns.
    pub fn add_member(&self, group: i64, friend: i64) -> Result<()> {
        self.request(Request::AddMember { group, friend })
    }

    /// Accepts an invitation to `group`.
    pub fn join_group(&self, group: i64) -> Result<()> {
        self.request(Request::JoinGroup(group))
    }

    pub fn leave_group(&self, group: i64) -> Result<()> {
        self.request(Request::LeaveGroup(group))
    }

    /// The user first, then everyone still in the group.
    pub fn members(&self, group: i64) -> Result<Vec<GroupMember>> {
        let Answer::Event(Event::Members { members, .. }) =
            self.ask(Want::Members(group), Request::Members(group))?
        else {
            return Err(unexpected(RequestKind::Members));
        };
        Ok(members)
    }
}
