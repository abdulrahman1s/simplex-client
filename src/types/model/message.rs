//! One message and its parts.

use serde::{Deserialize, Serialize};

use super::{ChatRef, LinkInfo, MessageBody};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", content = "c", rename_all = "snake_case")]
pub enum MessageAuthor {
    Me,
    Friend,
    Member(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Delivery {
    Sending,
    Sent,
    Delivered,
    /// Delivered, but the peer's receipt says the message it got hashes differently
    /// (`CISSndRcvd MRBadMsgHash`).
    DeliveredBadHash,
    Failed,
    Received,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Quote {
    pub id: Option<i64>,
    pub author: MessageAuthor,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reaction {
    pub emoji: String,
    pub count: u32,
    pub mine: bool,
    pub who: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mention {
    pub name: String,
    pub member: i64,
    pub me: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: i64,
    pub chat: ChatRef,
    pub author: MessageAuthor,
    pub body: MessageBody,
    /// Milliseconds since the epoch.
    pub at: i64,
    pub delivery: Delivery,
    pub unread: bool,
    pub reply: Option<Quote>,
    pub reactions: Vec<Reaction>,
    pub edited: bool,
    pub mentions: Vec<Mention>,
    /// A disappearing message: how long it lasts and when it goes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timed: Option<Timed>,
    /// The preview of the link the text has (`MCLink`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub link: Option<LinkInfo>,
}

/// A disappearing message (Messages.hs `CITimed`). The user's own count from sending; a
/// received one's count starts when it is read, so until then `delete_at` is `None`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timed {
    /// Seconds.
    pub ttl: i64,
    /// Milliseconds since the epoch.
    pub delete_at: Option<i64>,
}

impl ChatMessage {
    /// What the sender wrote: the text, or a file's caption; empty for notices, calls and the rest.
    #[must_use]
    pub fn text(&self) -> &str {
        match &self.body {
            MessageBody::Text(t) => t,
            MessageBody::Image(m)
            | MessageBody::Video(m)
            | MessageBody::File(m)
            | MessageBody::Voice(m) => &m.caption,
            MessageBody::Notice(_)
            | MessageBody::Gif(_)
            | MessageBody::Call(_)
            | MessageBody::Stream(_)
            | MessageBody::Deleted => "",
        }
    }

    /// Sent by this client's user.
    #[must_use]
    pub fn mine(&self) -> bool {
        self.author == MessageAuthor::Me
    }
}
