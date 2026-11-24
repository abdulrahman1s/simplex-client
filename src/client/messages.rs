//! Messages: send, reply, edit, delete, react, files, read marks and history.

use std::path::Path;

use super::error::{Result, unexpected};
use super::handle::Client;
use super::waiters::{Answer, Want};
use crate::types::model::{ChatMessage, ChatPrefs, ChatRef, LinkInfo};
use crate::types::{Event, FileKind, Request, RequestKind};

impl Client {
    /// Sends `text` to a friend or a group.
    pub fn send(&self, chat: ChatRef, text: &str) -> Result<()> {
        self.request(Request::Send {
            chat,
            text: text.to_owned(),
            reply_to: None,
            mentions: Vec::new(),
        })
    }

    /// Answers `msg` in its conversation, quoting it.
    pub fn reply(&self, msg: &ChatMessage, text: &str) -> Result<()> {
        self.request(Request::Send {
            chat: msg.chat,
            text: text.to_owned(),
            reply_to: Some(msg.id),
            mentions: Vec::new(),
        })
    }

    /// Changes the text of one of the user's own messages.
    pub fn edit(&self, msg: &ChatMessage, text: &str) -> Result<()> {
        self.request(Request::Edit {
            chat: msg.chat,
            item: msg.id,
            text: text.to_owned(),
        })
    }

    pub fn delete(&self, msg: &ChatMessage) -> Result<()> {
        self.request(Request::Delete {
            chat: msg.chat,
            item: msg.id,
        })
    }

    /// Adds a reaction; the engine carries the emoji in `types::REACTION_EMOJI`.
    pub fn react(&self, msg: &ChatMessage, emoji: &str) -> Result<()> {
        self.set_reaction(msg, emoji, true)
    }

    pub fn unreact(&self, msg: &ChatMessage, emoji: &str) -> Result<()> {
        self.set_reaction(msg, emoji, false)
    }

    fn set_reaction(&self, msg: &ChatMessage, emoji: &str, on: bool) -> Result<()> {
        self.request(Request::React {
            chat: msg.chat,
            item: msg.id,
            emoji: emoji.to_owned(),
            on,
        })
    }

    /// Sends a file; its progress arrives as `Event::MessageUpdate`.
    pub fn send_file(&self, chat: ChatRef, path: impl AsRef<Path>) -> Result<()> {
        self.request(Request::SendFile {
            chat,
            path: path.as_ref().to_owned(),
            kind: FileKind::File,
            preview: None,
            duration: None,
            text: String::new(),
        })
    }

    /// Downloads the file `msg` offers into the client's folder.
    pub fn receive_file(&self, msg: &ChatMessage) -> Result<()> {
        self.request(Request::ReceiveFile {
            chat: msg.chat,
            item: msg.id,
        })
    }

    pub fn mark_read(&self, chat: ChatRef) -> Result<()> {
        self.request(Request::MarkRead(chat))
    }

    /// Disappearing messages in `chat`: new ones last `ttl` seconds, or `None` to turn it off.
    pub fn set_timer(&self, chat: ChatRef, ttl: Option<i64>) -> Result<()> {
        self.request(Request::SetTimer { chat, ttl })
    }

    /// `text` with the preview of the link it has.
    pub fn send_link(&self, chat: ChatRef, text: &str, preview: LinkInfo) -> Result<()> {
        self.request(Request::SendLink {
            chat,
            text: text.to_owned(),
            preview,
            reply_to: None,
        })
    }

    /// The user's chat preferences, sent to every friend they change (`Request::SetPrefs`).
    pub fn set_prefs(&self, prefs: ChatPrefs) -> Result<()> {
        self.request(Request::SetPrefs {
            friend: None,
            prefs,
        })
    }

    /// Up to `count` messages before the message `before` (or the newest), oldest first.
    pub fn history(
        &self,
        chat: ChatRef,
        before: Option<i64>,
        count: u32,
    ) -> Result<Vec<ChatMessage>> {
        let tag = self.next_tag();
        let request = Request::History {
            chat,
            before,
            count,
            tag,
        };
        let Answer::Event(Event::History { messages, .. }) =
            self.ask(Want::History(tag), request)?
        else {
            return Err(unexpected(RequestKind::History));
        };
        Ok(messages)
    }
}
