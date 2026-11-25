//! Where a message is, and who wrote it.

use super::{Friend, Group, Member};
use crate::client::{Result, SendHandle};
use crate::types::model::ChatRef;

/// A conversation: with a friend, or a group.
#[derive(Clone, Debug)]
pub enum Chat {
    Friend(Friend),
    Group(Group),
}

impl Chat {
    pub fn send(&self, text: &str) -> Result<SendHandle> {
        match self {
            Self::Friend(f) => f.send(text),
            Self::Group(g) => g.send(text),
        }
    }

    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Friend(f) => f.name(),
            Self::Group(g) => g.name(),
        }
    }

    #[must_use]
    pub const fn chat(&self) -> ChatRef {
        match self {
            Self::Friend(f) => f.chat(),
            Self::Group(g) => g.chat(),
        }
    }
}

/// Who wrote a message.
#[derive(Clone, Debug)]
pub enum Author {
    /// The user.
    Me,
    Friend(Friend),
    Member(Member),
}
