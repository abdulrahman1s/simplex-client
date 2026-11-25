//! The timer a new item takes: the user's own counts from now; a received one's from when it is read.

use crate::types::model::{ChatRef, Timed};

use super::rules::{friend_ttl, group_ttl, received, sent};
use crate::engine::agent_port::AgentPort;
use crate::engine::error::Result;
use crate::engine::machine::Engine;

impl<A: AgentPort> Engine<A> {
    /// `contactTimedTTL` or `groupTimedTTL` of the chat.
    pub(crate) fn chat_ttl(&self, chat: ChatRef) -> Result<Option<Option<i64>>> {
        Ok(match chat {
            ChatRef::Direct(f) => friend_ttl(self.state.user.prefs.as_ref(), &self.friend(f)?),
            ChatRef::Group(g) => group_ttl(&self.group(g)?),
        })
    }

    /// The user's new item in `chat`; its ttl goes in the message too.
    pub(crate) fn outgoing_timed(&self, chat: ChatRef) -> Result<Option<Timed>> {
        Ok(sent(self.chat_ttl(chat)?, self.clock.wall_ms()))
    }

    /// A received item carrying `ttl`. A forwarded group message keeps its ttl whatever the
    /// group's preference (Subscriber.hs `rcvCITimed_ (Just Nothing)`).
    pub(crate) fn incoming_timed(
        &self,
        chat: ChatRef,
        ttl: Option<i64>,
        forwarded: bool,
    ) -> Result<Option<Timed>> {
        let chat_ttl = if forwarded {
            Some(None)
        } else {
            self.chat_ttl(chat)?
        };
        Ok(received(chat_ttl, ttl))
    }
}
