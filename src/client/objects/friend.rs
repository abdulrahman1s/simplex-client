//! A friend: write to them, read your history, compare security codes, remove them.

use super::Message;
use crate::client::{Client, Result};
use std::collections::BTreeMap;

use crate::types::Request;
use crate::types::model::{self as wire, ChatFeature, ChatPrefs, ChatRef, PrefEnabled};

#[derive(Clone, Debug)]
pub struct Friend {
    client: Client,
    info: wire::Friend,
}

impl Friend {
    pub(in crate::client) const fn new(client: Client, info: wire::Friend) -> Self {
        Self { client, info }
    }

    #[must_use]
    pub const fn id(&self) -> i64 {
        self.info.id
    }

    /// Empty when the chat list did not have this friend yet.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.info.name
    }

    /// Connected: messages can be sent.
    #[must_use]
    pub const fn ready(&self) -> bool {
        self.info.ready
    }

    /// Everything the chat list says about this friend.
    #[must_use]
    pub const fn info(&self) -> &wire::Friend {
        &self.info
    }

    #[must_use]
    pub const fn chat(&self) -> ChatRef {
        self.info.chat()
    }

    pub fn send(&self, text: &str) -> Result<()> {
        self.client.send(self.chat(), text)
    }

    /// The connection is deleted on both sides; the conversation goes with it.
    pub fn remove(&self) -> Result<()> {
        self.client.remove_friend(self.id())
    }

    /// The security code to compare with the friend's, out of band.
    pub fn show_code(&self) -> Result<String> {
        self.client.show_code(self.id())
    }

    /// The newest `count` messages, oldest first.
    pub fn history(&self, count: u32) -> Result<Vec<Message>> {
        Message::page(&self.client, self.chat(), count)
    }

    pub fn mark_read(&self) -> Result<()> {
        self.client.mark_read(self.chat())
    }

    /// Seconds new messages last, when disappearing messages are on for both sides.
    #[must_use]
    pub const fn timer(&self) -> Option<i64> {
        self.info.timer
    }

    /// Offers the friend disappearing messages after `ttl` seconds, or turns them off for
    /// both sides with `None`.
    pub fn set_timer(&self, ttl: Option<i64>) -> Result<()> {
        self.client.set_timer(self.chat(), ttl)
    }

    /// The user's preferences for this friend, over their own; a feature left out follows them.
    pub fn set_prefs(&self, prefs: ChatPrefs) -> Result<()> {
        self.client.request(Request::SetPrefs {
            friend: Some(self.id()),
            prefs,
        })
    }

    /// What is on between the user and this friend, feature by feature.
    #[must_use]
    pub const fn features(&self) -> &BTreeMap<ChatFeature, PrefEnabled> {
        &self.info.features
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

    /// Sends `msg` here as the user's own, marked forwarded; its file only when it is here.
    pub fn forward(&self, msg: &Message) -> Result<()> {
        self.client.forward(msg.info(), self.chat())
    }

    /// Starts a live message here, seen as it is written.
    pub fn send_live(&self, text: &str) -> Result<()> {
        self.client.send_live(self.chat(), text)
    }

    /// "Fix connection": a new ratchet with them; `force` asks even while it works.
    pub fn fix_connection(&self, force: bool) -> Result<()> {
        self.client.request(Request::SyncRatchet {
            chat: self.chat(),
            member: None,
            force,
        })
    }
}
