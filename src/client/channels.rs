//! Channels from the caller's side: the chat relays to use, a new channel, posting signed.
//!
//! A channel is not end-to-end encrypted: every owner→relay and relay→subscriber connection
//! is made without the post-quantum KEM, as simplex-chat 7.1.4 makes them, and the relays read
//! and keep everything posted (they replay the last 100 posts to new subscribers).
//! [`ChannelRelay::pq`](crate::types::model::ChannelRelay::pq) shows each connection's state.

use crate::client::{Client, Result, SendHandle};
use crate::types::model::{ChatRef, ChatRelay};
use crate::types::{Request, RequestKind};

use super::objects::Group;
use super::waiters::{Answer, Want};

impl Client {
    /// The chat relays new channels are made on, replacing any set before. Only these are
    /// used: no preset relay is ever chosen for the caller.
    pub fn set_chat_relays(&self, relays: Vec<ChatRelay>) -> Result<()> {
        self.request(Request::SetChatRelays(relays))
    }

    /// Makes a channel owned by the user on the relays [`Client::set_chat_relays`] gave and
    /// waits until the chat list shows it. Each relay is invited, tested and accepted after
    /// this returns; [`Event::Raw`](super::Event::Raw) with `ChannelRelay` tells each change,
    /// and the group's `channel` the current state. The channel's link is `channel.link`.
    ///
    /// Relays see every post in the clear and keep them; the connections have no PQ KEM.
    pub fn create_channel(&self, name: &str) -> Result<Group> {
        let Answer::Group(g) = self.ask(Want::NewGroup, Request::NewChannel(name.to_owned()))?
        else {
            return Err(super::error::unexpected(RequestKind::Group));
        };
        Ok(Group::new(self.clone(), g))
    }

    /// [`Client::send_data`] signed with the user's member key: in a channel, a post as the
    /// channel that subscribers verify against the owner's key. A signed body over 15 602
    /// bytes is refused (`TextTooLong`) before it is sent. Relays still read it, and they
    /// forward a signed post naming the owner's member, not as the channel alone.
    pub fn send_signed_data(
        &self,
        chat: ChatRef,
        tag: &str,
        json: serde_json::Map<String, serde_json::Value>,
        fallback_text: &str,
    ) -> Result<SendHandle> {
        self.tagged(Request::SendSignedData {
            chat,
            tag: tag.to_owned(),
            json,
            fallback_text: fallback_text.to_owned(),
        })
    }
}
