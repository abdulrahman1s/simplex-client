//! What the user does to conversations for themselves alone: deletions here, marks, aliases, message TTLs.

use super::error::Result;
use super::handle::Client;
use crate::types::Request;
use crate::types::model::{ChatMessage, ChatRef};

impl Client {
    /// Deletes a message here only, with its file.
    pub fn delete_for_me(&self, msg: &ChatMessage) -> Result<()> {
        self.request(Request::DeleteForMe {
            chat: msg.chat,
            item: msg.id,
        })
    }

    /// Reads `msg`'s conversation up to and including it.
    pub fn mark_read_up_to(&self, msg: &ChatMessage) -> Result<()> {
        self.request(Request::MarkReadUpTo {
            chat: msg.chat,
            item: msg.id,
        })
    }

    /// Deletes every message of the conversation here, with their files.
    pub fn clear_chat(&self, chat: ChatRef) -> Result<()> {
        self.request(Request::ClearChat(chat))
    }

    pub fn set_unread_mark(&self, chat: ChatRef, on: bool) -> Result<()> {
        self.request(Request::SetUnreadMark { chat, on })
    }

    /// The user's own name for a friend or group, never sent; empty takes it off.
    pub fn set_alias(&self, chat: ChatRef, alias: &str) -> Result<()> {
        self.request(Request::SetAlias {
            chat,
            alias: alias.to_owned(),
        })
    }

    /// Messages older than `ttl` seconds are deleted here: for every chat with `chat: None`,
    /// or one chat (`None` follows the user's own, `Some(0)` keeps them).
    pub fn set_message_ttl(&self, chat: Option<ChatRef>, ttl: Option<i64>) -> Result<()> {
        self.request(Request::SetMessageTtl { chat, ttl })
    }
}
