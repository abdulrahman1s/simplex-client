//! Channels from the caller's side: the chat relays to use, a new channel, posting signed.
//!
//! A channel is not end-to-end encrypted: every owner→relay and relay→subscriber connection
//! is made without the post-quantum KEM, as simplex-chat 7.1.4 makes them, and the relays read
//! and keep everything posted (they replay the last 100 posts to new subscribers).
//! [`ChannelRelay::pq`](crate::types::model::ChannelRelay::pq) shows each connection's state.

use crate::client::{Client, Result, SendHandle};
use crate::types::model::{ChatRef, ChatRelay, RelayTestResult};
use crate::types::{Request, RequestKind};

use super::objects::Group;
use super::waiters::{Answer, Want};

impl Client {
    /// The chat relays new channels are made on, replacing any set before. Only these are
    /// used: no preset relay is ever chosen for the caller.
    pub fn set_chat_relays(&self, relays: Vec<ChatRelay>) -> Result<()> {
        self.request(Request::SetChatRelays(relays))
    }

    /// Tests a chat relay's address as 7.1.4's `/relay test` does: its link is read, joined
    /// with a random challenge, and the relay must answer with the challenge signed by the
    /// address's key within 40 seconds. The test connection is deleted either way. The
    /// client's timeout must be longer than 40 seconds to hear a timed-out test.
    pub fn test_chat_relay(&self, address: &str) -> Result<RelayTestResult> {
        let answer = self.ask(Want::RelayTest, Request::TestChatRelay(address.to_owned()))?;
        let Answer::Event(crate::types::Event::RelayTest(r)) = answer else {
            return Err(super::error::unexpected(RequestKind::RelayTest));
        };
        Ok(r)
    }

    /// Invites more chat relays to a channel the user owns (`APIAddGroupRelays`). Each goes
    /// through invitation, acceptance and the roster as at creation, and the channel's link
    /// is published again naming it; its status changes come as `Event::Raw(ChannelRelay)`.
    /// Subscribers who join later join it too; while the channel has subscribers, the relays
    /// already serving it announce it with a signed `x.grp.relay.new`. Its connections, like
    /// every channel connection, have no post-quantum KEM.
    pub fn add_channel_relays(&self, group: i64, relays: Vec<ChatRelay>) -> Result<()> {
        self.request(Request::AddChannelRelays { group, relays })
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
    /// forward a signed post naming the owner's member, not as the channel alone. As with
    /// [`Client::send_data`], an empty `fallback_text` makes the post one relays never replay
    /// to later subscribers.
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
