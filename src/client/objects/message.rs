//! A message: answer it, react to it, change or delete it.

use super::{Author, Chat, Member};
use crate::client::{Client, Error, Result};
use crate::types::model::{ChatMessage, ChatRef, MessageAuthor};
use crate::types::{ErrorCode, Request, RequestKind};

#[derive(Clone, Debug)]
pub struct Message {
    client: Client,
    info: ChatMessage,
}

impl Message {
    pub(in crate::client) const fn new(client: Client, info: ChatMessage) -> Self {
        Self { client, info }
    }

    /// The newest `count` messages of `chat`, oldest first.
    pub(super) fn page(client: &Client, chat: ChatRef, count: u32) -> Result<Vec<Self>> {
        let list = client.history(chat, None, count)?;
        let wrap = |m| Self::new(client.clone(), m);
        Ok(list.into_iter().map(wrap).collect())
    }

    #[must_use]
    pub const fn id(&self) -> i64 {
        self.info.id
    }

    /// What the sender wrote: the text, or a file's caption; empty for notices and calls.
    #[must_use]
    pub fn text(&self) -> &str {
        self.info.text()
    }

    /// A disappearing message: its ttl, and when it goes (a received one once it is read).
    #[must_use]
    pub const fn timed(&self) -> Option<crate::types::model::Timed> {
        self.info.timed
    }

    /// Sent by the user.
    #[must_use]
    pub fn mine(&self) -> bool {
        self.info.mine()
    }

    /// The whole message: body, time, delivery, reactions, quote, mentions.
    #[must_use]
    pub const fn info(&self) -> &ChatMessage {
        &self.info
    }

    #[must_use]
    pub fn chat(&self) -> Chat {
        match self.info.chat {
            ChatRef::Direct(id) => Chat::Friend(self.client.friend_or_id(id)),
            ChatRef::Group(id) => Chat::Group(self.client.group_or_id(id)),
        }
    }

    #[must_use]
    pub fn author(&self) -> Author {
        match (&self.info.author, self.info.chat) {
            (MessageAuthor::Me, _) => Author::Me,
            (MessageAuthor::Friend | MessageAuthor::Member(_), ChatRef::Direct(id)) => {
                Author::Friend(self.client.friend_or_id(id))
            }
            (MessageAuthor::Friend, ChatRef::Group(g)) => {
                Author::Member(Member::named(self.client.clone(), g, String::new()))
            }
            (MessageAuthor::Member(name), ChatRef::Group(g)) => {
                Author::Member(Member::named(self.client.clone(), g, name.clone()))
            }
        }
    }

    /// Answers in the same conversation, quoting this message.
    pub fn reply(&self, text: &str) -> Result<()> {
        self.client.reply(&self.info, text)
    }

    /// Adds a reaction; the engine carries the emoji in `types::REACTION_EMOJI`.
    pub fn react(&self, emoji: &str) -> Result<()> {
        self.client.react(&self.info, emoji)
    }

    pub fn unreact(&self, emoji: &str) -> Result<()> {
        self.client.unreact(&self.info, emoji)
    }

    /// Changes the text; only the user's own messages.
    pub fn edit(&self, text: &str) -> Result<()> {
        self.client.edit(&self.info, text)
    }

    pub fn delete(&self) -> Result<()> {
        self.client.delete(&self.info)
    }

    /// The preview of the link the text has.
    #[must_use]
    pub const fn link(&self) -> Option<&crate::types::model::LinkInfo> {
        self.info.link.as_ref()
    }

    /// Deletes this message here only, with its file.
    pub fn delete_for_me(&self) -> Result<()> {
        self.client.delete_for_me(&self.info)
    }

    /// Reads the conversation up to and including this message.
    pub fn mark_read_up_to(&self) -> Result<()> {
        self.client.mark_read_up_to(&self.info)
    }

    /// A moderator deletes a member's message in a group for everyone.
    pub fn moderate(&self) -> Result<()> {
        let ChatRef::Group(group) = self.info.chat else {
            return Err(Error::Failed {
                request: RequestKind::GroupAdmin,
                code: ErrorCode::RequestFailed,
                detail: "only a group's message is moderated".into(),
            });
        };
        self.client.request(Request::Moderate {
            group,
            item: self.info.id,
        })
    }

    /// Downloads the file this message offers into the client's folder.
    pub fn receive_file(&self) -> Result<()> {
        self.client.receive_file(&self.info)
    }
}
