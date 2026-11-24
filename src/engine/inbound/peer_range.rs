//! A friend's chat version range, kept from what they send, and PQ on for their connection (Internal.hs `updatePeerChatVRange`).

use crate::chat::{ChatEvent, ChatMessage, parse_chat_messages};
use crate::engine::agent_port::AgentPort;
use crate::engine::error::Result;
use crate::engine::machine::Engine;

impl<A: AgentPort> Engine<A> {
    /// A binary file chunk carries no range. 7.1.4 turns PQ on here for every contact
    /// connection whatever the range (its "v6.0 remove/review" TODO); the agent turns the KEM
    /// on with the next message sent.
    pub(crate) fn refresh_friend_range(&mut self, friend: i64, msg: &ChatMessage) -> Result<()> {
        if matches!(msg.event, ChatEvent::FileChunk { .. }) {
            return Ok(());
        }
        let mut f = self.friend(friend)?;
        let range = Some(msg.version_range());
        if f.chat_v == range && f.pq {
            return Ok(());
        }
        f.chat_v = range;
        f.pq = true;
        Ok(self.save_friend(f)?)
    }

    /// The same for the conn-info of a `CONF` or `INFO`.
    pub(crate) fn refresh_friend_range_from(&mut self, friend: i64, info: &[u8]) -> Result<()> {
        let msgs = parse_chat_messages(info).unwrap_or_default();
        match msgs.first() {
            Some(msg) => self.refresh_friend_range(friend, msg),
            None => Ok(()),
        }
    }
}
