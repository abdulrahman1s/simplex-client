//! SetMessageTtl: messages older than a number of seconds are deleted here, for every chat or one (Commands.hs `APISetChatItemTTL`, `APISetChatTTL`).

use crate::types::model::ChatRef;

use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::machine::Engine;

impl<A: AgentPort> Engine<A> {
    /// What is old now goes at once; the rest as it gets old (`setChatItemsExpiration`).
    pub(crate) fn set_message_ttl(
        &mut self,
        chat: Option<ChatRef>,
        ttl: Option<i64>,
    ) -> Result<()> {
        if ttl.is_some_and(|t| t < 0) {
            return Err(fail(Code::RequestFailed, "a message TTL is never negative"));
        }
        match chat {
            None => {
                let mut s = self.state.settings.clone();
                s.item_ttl = ttl.unwrap_or(0);
                self.save_settings(s)?;
                self.emit(crate::types::Event::Profile(self.profile_wire()));
            }
            Some(chat) => self.change_local(chat, |l| l.item_ttl = ttl)?,
        }
        self.state.next_expire = Some(self.clock.wall_ms());
        Ok(())
    }

    /// The seconds after which `chat`'s messages go, if they do.
    pub(crate) fn ttl_of(&self, chat: ChatRef) -> Option<i64> {
        let own = self.local_of(chat).ok()?.item_ttl;
        let ttl = own.unwrap_or(self.state.settings.item_ttl);
        (ttl > 0).then_some(ttl)
    }
}
