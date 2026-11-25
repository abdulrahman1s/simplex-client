//! Timed items whose time came are deleted with their files (Internal.hs `deleteTimedItem`, `deleteDirectCIs`; the cleanup manager's `getTimedItems` at start).

use crate::types::Event;
use crate::types::model::ChatRef;

use super::keys::timed_due;

use crate::engine::agent_port::AgentPort;
use crate::engine::machine::Engine;
use crate::engine::records::NS;

/// The most items one tick deletes, so a backlog after a long time offline goes out in
/// batches and the writer's outbox stays near `OUT_CAP`.
const PER_TICK: usize = 256;

impl<A: AgentPort> Engine<A> {
    /// The earliest wall ms at which a timed item goes.
    #[must_use]
    pub(crate) fn next_timed_wake(&self) -> Option<i64> {
        self.state.timed.first().map(|(at, _, _)| *at)
    }

    /// Deletes the items due by `now`, telling the page in one event.
    pub(crate) fn expire_timed(&mut self, now: i64) {
        let due: Vec<_> = (self.state.timed.iter())
            .take_while(|(at, _, _)| *at <= now)
            .take(PER_TICK)
            .copied()
            .collect();
        let mut gone = Vec::new();
        for (at, chat, item) in due {
            let found = match self.item(chat, item) {
                Ok(found) => found,
                Err(e) => {
                    self.log
                        .line(format_args!("a timed item was not read: {e}"));
                    None
                }
            };
            let Some(it) = found else {
                // Gone already: only its entry is left.
                self.drop_entry(at, chat, item);
                continue;
            };
            let done = self
                .drop_item_file(chat, item)
                .and_then(|()| self.remove_item(chat, item, &it));
            match done {
                Ok(m) => gone.push(m),
                Err(e) => {
                    self.log
                        .line(format_args!("a timed item was not deleted: {}", e.detail));
                    // Not tried again every tick: it stays in the store for the next start.
                    self.state.timed.remove(&(at, chat, item));
                }
            }
        }
        if !gone.is_empty() {
            self.state.chats_dirty = true;
            self.emit(Event::Updated(gone));
        }
    }
}

impl<A: AgentPort> Engine<A> {
    fn drop_entry(&mut self, at: i64, chat: ChatRef, item: i64) {
        self.state.timed.remove(&(at, chat, item));
        if let Err(e) = self.store.delete(NS, &timed_due(at, chat, item)) {
            self.log
                .line(format_args!("a timed entry was not deleted: {e}"));
        }
    }
}
