//! Keeping the timed index with the items: a new timed item is entered, a deleted one leaves, and a chat's go with it.

// The store's prefix scans open every row of the namespace, so the index is read from the
// store once at start and kept in memory; the store's copy is for the next start.

use crate::types::model::{ChatRef, Timed};

use super::keys::{
    TIMED_DUE_PREFIX, TIMED_UNREAD_PREFIX, parse_timed_due, parse_timed_unread, timed_due,
    timed_unread,
};
use crate::engine::agent_port::AgentPort;
use crate::engine::machine::Engine;
use crate::engine::records::NS;
use crate::store::{Store, StoreError};

/// The delete times the store holds, for the state at start.
pub fn load_due(store: &mut Store) -> Result<Vec<(i64, ChatRef, i64)>, StoreError> {
    let keys = store.keys_with_prefix(NS, TIMED_DUE_PREFIX)?;
    Ok(keys.iter().filter_map(|k| parse_timed_due(k)).collect())
}

/// The received timed items not read yet, for the state at start.
pub fn load_unread(store: &mut Store) -> Result<Vec<(ChatRef, i64)>, StoreError> {
    let keys = store.keys_with_prefix(NS, TIMED_UNREAD_PREFIX)?;
    Ok(keys.iter().filter_map(|k| parse_timed_unread(k)).collect())
}

impl<A: AgentPort> Engine<A> {
    /// A new item's timer: one with a delete time is due then; a received one waits to be read.
    pub(crate) fn enter_timed(
        &mut self,
        chat: ChatRef,
        item: i64,
        timed: Option<Timed>,
    ) -> Result<(), StoreError> {
        match timed {
            Some(Timed {
                delete_at: Some(at),
                ..
            }) => self.schedule(chat, item, at),
            Some(Timed {
                delete_at: None, ..
            }) => {
                self.store.put(NS, &timed_unread(chat, item), b"")?;
                self.state.timed_unread.insert((chat, item));
                Ok(())
            }
            None => Ok(()),
        }
    }

    pub(crate) fn schedule(&mut self, chat: ChatRef, item: i64, at: i64) -> Result<(), StoreError> {
        self.store.put(NS, &timed_due(at, chat, item), b"")?;
        self.state.timed.insert((at.max(0), chat, item));
        Ok(())
    }

    /// No longer waiting to be read: read, or gone.
    pub(crate) fn unread_done(&mut self, chat: ChatRef, item: i64) -> Result<(), StoreError> {
        self.state.timed_unread.remove(&(chat, item));
        self.store.delete(NS, &timed_unread(chat, item))
    }

    /// The item went some other way: its timer goes too.
    pub(crate) fn leave_timed(
        &mut self,
        chat: ChatRef,
        item: i64,
        timed: Option<Timed>,
    ) -> Result<(), StoreError> {
        let Some(t) = timed else { return Ok(()) };
        match t.delete_at {
            Some(at) => {
                self.state.timed.remove(&(at.max(0), chat, item));
                self.store.delete(NS, &timed_due(at, chat, item))
            }
            None => self.unread_done(chat, item),
        }
    }

    /// The chat is deleted: none of its timers are left.
    pub(crate) fn forget_timed(&mut self, chat: ChatRef) -> Result<(), StoreError> {
        let unread: Vec<_> = self
            .state
            .timed_unread
            .range(chat_range(chat))
            .copied()
            .collect();
        for (c, item) in unread {
            self.unread_done(c, item)?;
        }
        let due: Vec<_> = (self.state.timed.iter())
            .filter(|(_, c, _)| *c == chat)
            .copied()
            .collect();
        for (at, c, item) in due {
            self.state.timed.remove(&(at, c, item));
            self.store.delete(NS, &timed_due(at, c, item))?;
        }
        Ok(())
    }
}

/// Every (chat, item) entry of one chat.
pub(super) const fn chat_range(chat: ChatRef) -> std::ops::RangeInclusive<(ChatRef, i64)> {
    (chat, i64::MIN)..=(chat, i64::MAX)
}
