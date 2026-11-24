//! A group: write to it, invite friends, list members, leave.

use super::{Friend, Member, Message};
use crate::client::{Client, Result};
use crate::types::Request;
use crate::types::model::{ChatGroup, ChatRef, GroupPrefs};

#[derive(Clone, Debug)]
pub struct Group {
    client: Client,
    info: ChatGroup,
}

impl Group {
    pub(in crate::client) const fn new(client: Client, info: ChatGroup) -> Self {
        Self { client, info }
    }

    #[must_use]
    pub const fn id(&self) -> i64 {
        self.info.id
    }

    /// Empty when the chat list did not have this group yet.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.info.name
    }

    /// Everything the chat list says about this group.
    #[must_use]
    pub const fn info(&self) -> &ChatGroup {
        &self.info
    }

    #[must_use]
    pub const fn chat(&self) -> ChatRef {
        self.info.chat()
    }

    pub(super) const fn client(&self) -> &Client {
        &self.client
    }

    pub fn send(&self, text: &str) -> Result<()> {
        self.client.send(self.chat(), text)
    }

    /// Invites a friend; the user must own the group.
    pub fn add_member(&self, friend: &Friend) -> Result<()> {
        self.client.add_member(self.id(), friend.id())
    }

    pub fn leave(&self) -> Result<()> {
        self.client.leave_group(self.id())
    }

    /// The user first, then everyone still in the group.
    pub fn members(&self) -> Result<Vec<Member>> {
        let list = self.client.members(self.id())?;
        let id = self.id();
        let member = |m| Member::listed(self.client.clone(), id, m);
        Ok(list.into_iter().map(member).collect())
    }

    /// The newest `count` messages, oldest first.
    pub fn history(&self, count: u32) -> Result<Vec<Message>> {
        Message::page(&self.client, self.chat(), count)
    }

    pub fn mark_read(&self) -> Result<()> {
        self.client.mark_read(self.chat())
    }

    /// Seconds new messages last, when the group has disappearing messages on.
    #[must_use]
    pub const fn timer(&self) -> Option<i64> {
        self.info.timer
    }

    /// The group's disappearing messages: `ttl` seconds, or `None` for off. Owners only.
    pub fn set_timer(&self, ttl: Option<i64>) -> Result<()> {
        self.client.set_timer(self.chat(), ttl)
    }

    /// Deletes the group here; an owner deletes it for every member too.
    pub fn delete(&self) -> Result<()> {
        self.client.request(Request::DeleteGroup(self.id()))
    }

    /// An owner renames the group or changes its description.
    pub fn set_profile(&self, name: &str, description: Option<&str>) -> Result<()> {
        self.client.request(Request::SetGroupProfile {
            group: self.id(),
            name: name.to_owned(),
            description: description.map(str::to_owned),
        })
    }

    /// An owner turns group features on or off; one left out stays.
    pub fn set_prefs(&self, prefs: GroupPrefs) -> Result<()> {
        self.client.request(Request::SetGroupPrefs {
            group: self.id(),
            prefs,
        })
    }

    /// Deletes every message here, with their files.
    pub fn clear(&self) -> Result<()> {
        self.client.clear_chat(self.chat())
    }

    pub fn set_unread_mark(&self, on: bool) -> Result<()> {
        self.client.set_unread_mark(self.chat(), on)
    }

    /// The user's own name, never sent; empty takes it off.
    pub fn set_alias(&self, alias: &str) -> Result<()> {
        self.client.set_alias(self.chat(), alias)
    }

    /// Messages older than `ttl` seconds are deleted here; `None` follows the user's own
    /// setting and `Some(0)` keeps them.
    pub fn set_message_ttl(&self, ttl: Option<i64>) -> Result<()> {
        self.client.set_message_ttl(Some(self.chat()), ttl)
    }
}
