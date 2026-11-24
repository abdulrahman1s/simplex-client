//! A friend: write to them, read your history, compare security codes, remove them.

use super::Message;
use crate::client::{Client, Result};
use crate::types::model::{self as wire, ChatRef};

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
}
