//! SetChatRelays: the caller's chat relays, the only ones new channels are made on; AddChannelRelays: more of them for a channel the user owns (Commands.hs `SetUserChatRelays`, `APIAddGroupRelays`).

use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::machine::Engine;
use crate::types::model::ChatRelay;

impl<A: AgentPort> Engine<A> {
    /// `SetUserChatRelays`: replaces the list; an address that is not a contact short link is refused.
    pub(crate) fn set_chat_relays(&mut self, relays: Vec<ChatRelay>) -> Result<()> {
        for r in &relays {
            check_address(&r.address)?;
        }
        let mut s = self.state.settings.clone();
        s.chat_relays = relays;
        self.save_settings(s)?;
        Ok(())
    }

    /// `APIAddGroupRelays`: the user owns the channel, and none of `relays` serves it yet. Each
    /// is invited, accepted, given the roster and published as at creation; the link data is
    /// set again with every relay that may serve.
    pub(crate) fn add_channel_relays(&mut self, group: i64, relays: &[ChatRelay]) -> Result<()> {
        let g = self.group(group)?;
        let Some(c) = g.channel.as_ref() else {
            return Err(fail(Code::RequestFailed, "group does not use relays"));
        };
        if !c.owner() {
            return Err(fail(Code::NotAllowed, "groupUserRole: required role owner"));
        }
        if relays.is_empty() {
            return Err(fail(Code::RequestFailed, "no relays"));
        }
        let known = |a: &str| c.relays.iter().any(|r| r.address == a.trim());
        if relays.iter().any(|r| known(&r.address)) {
            return Err(fail(
                Code::RequestFailed,
                "some relays are already in the group",
            ));
        }
        for r in relays {
            check_address(&r.address)?;
        }
        self.invite_relays(group, relays)?;
        self.state.chats_dirty = true;
        Ok(())
    }
}

fn check_address(address: &str) -> Result<()> {
    let ok = matches!(
        crate::agent::links::ConnShortLink::parse(address.trim()),
        Ok(crate::agent::links::ConnShortLink::Contact { .. })
    );
    if ok {
        Ok(())
    } else {
        Err(fail(
            Code::BadLink,
            format!("not a chat relay address: {address}"),
        ))
    }
}
