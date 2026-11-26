//! SetChatRelays: the caller's chat relays, the only ones new channels are made on (Commands.hs `SetUserChatRelays`).

use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::machine::Engine;

impl<A: AgentPort> Engine<A> {
    /// `SetUserChatRelays`: replaces the list; an address that is not a contact short link is refused.
    pub(crate) fn set_chat_relays(
        &mut self,
        relays: Vec<crate::types::model::ChatRelay>,
    ) -> Result<()> {
        for r in &relays {
            let ok = matches!(
                crate::agent::links::ConnShortLink::parse(r.address.trim()),
                Ok(crate::agent::links::ConnShortLink::Contact { .. })
            );
            if !ok {
                return Err(fail(
                    Code::BadLink,
                    format!("not a chat relay address: {}", r.address),
                ));
            }
        }
        let mut s = self.state.settings.clone();
        s.chat_relays = relays;
        self.save_settings(s)?;
        Ok(())
    }
}
