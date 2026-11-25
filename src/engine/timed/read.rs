//! Reading a chat starts the count of its received timed items (Commands.hs `APIChatRead`; Store/Messages.hs `getDirectUnreadTimedItems`, `setDirectChatItemsDeleteAt`).

use crate::types::Event;
use crate::types::model::ChatRef;

use super::index::chat_range;
use super::rules::deadline;
use crate::engine::agent_port::AgentPort;
use crate::engine::error::Result;
use crate::engine::machine::Engine;

impl<A: AgentPort> Engine<A> {
    /// Every unread timed item up to `up_to` now counts down from this moment.
    pub(crate) fn start_read_timers(&mut self, chat: ChatRef, up_to: i64) -> Result<()> {
        let now = self.clock.wall_ms();
        let read: Vec<i64> = (self.state.timed_unread.range(chat_range(chat)))
            .map(|(_, item)| *item)
            .take_while(|item| *item <= up_to)
            .collect();
        let mut changed = Vec::new();
        for item in read {
            self.unread_done(chat, item)?;
            let Some(mut it) = self.item(chat, item)? else {
                continue;
            };
            let Some(t) = it.timed.as_mut().filter(|t| t.delete_at.is_none()) else {
                continue;
            };
            let at = deadline(now, t.ttl);
            t.delete_at = Some(at);
            let m = self.change_item(chat, item, &it)?;
            changed.push(self.shown_last(chat, &it, m));
            self.schedule(chat, item, at)?;
        }
        if !changed.is_empty() {
            self.emit(Event::Updated(changed));
        }
        Ok(())
    }
}
