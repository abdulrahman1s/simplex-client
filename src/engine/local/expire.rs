//! Old messages are deleted here as each chat's TTL says, every half hour (Commands.hs `expireChatItems`, `withExpirationDate`; Store/Messages.hs `deleteContactExpiredCIs`, `deleteGroupExpiredCIs`).

use crate::types::Event;
use crate::types::model::ChatRef;

use crate::engine::agent_port::AgentPort;
use crate::engine::error::Result;
use crate::engine::machine::Engine;
use crate::engine::records::{ItemRecord, decode, keys};

/// `ciExpirationInterval`.
const EVERY_MS: i64 = 30 * 60 * 1000;
/// A group's messages of the last 12 hours stay whatever they say of their time.
const GROUP_KEEP_MS: i64 = 12 * 60 * 60 * 1000;
/// The most items one tick deletes; what is left goes on the next.
const PER_TICK: usize = 256;
/// Items read per page.
const PAGE: u32 = 500;

impl<A: AgentPort> Engine<A> {
    /// When old messages are next looked for, while any chat has a TTL.
    #[must_use]
    pub(crate) fn next_expire_wake(&self) -> Option<i64> {
        let any = self.state.settings.item_ttl > 0
            || self
                .state
                .friends
                .values()
                .any(|f| f.local.item_ttl.is_some_and(|t| t > 0))
            || self
                .state
                .groups
                .values()
                .any(|g| g.local.item_ttl.is_some_and(|t| t > 0));
        any.then(|| self.state.next_expire.unwrap_or(0))
    }

    pub(crate) fn expire_old(&mut self, now: i64) {
        if self.next_expire_wake().is_none_or(|at| at > now) {
            return;
        }
        let chats: Vec<ChatRef> = (self.state.friends.keys().map(|f| ChatRef::Direct(*f)))
            .chain(self.state.groups.keys().map(|g| ChatRef::Group(*g)))
            .collect();
        let mut budget = PER_TICK;
        let mut gone = Vec::new();
        for chat in chats {
            let Some(ttl) = self.ttl_of(chat) else {
                continue;
            };
            let mut cutoff = now.saturating_sub(ttl.saturating_mul(1000));
            if let ChatRef::Group(_) = chat {
                cutoff = cutoff.min(now - GROUP_KEEP_MS);
            }
            match self.expire_chat(chat, cutoff, &mut budget, &mut gone) {
                Ok(()) => {}
                Err(e) => self
                    .log
                    .line(format_args!("old messages not deleted: {}", e.detail)),
            }
        }
        // A budget spent may leave more: look again on the next tick.
        self.state.next_expire = Some(if budget == 0 { now } else { now + EVERY_MS });
        if !gone.is_empty() {
            self.state.chats_dirty = true;
            self.emit(Event::Updated(gone));
        }
    }

    fn expire_chat(
        &mut self,
        chat: ChatRef,
        cutoff: i64,
        budget: &mut usize,
        gone: &mut Vec<crate::types::model::ChatMessage>,
    ) -> Result<()> {
        let mut before = None;
        let mut old = Vec::new();
        loop {
            let page = self.store.page(&keys::chat(chat), before, PAGE)?;
            let Some(&(oldest, _)) = page.first() else {
                break;
            };
            for (id, bytes) in page {
                let it: ItemRecord = decode(&bytes)?;
                if it.at <= cutoff {
                    old.push((id, it));
                }
            }
            before = Some(oldest);
        }
        for (id, it) in old.into_iter().take(*budget) {
            self.drop_item_file(chat, id)?;
            gone.push(self.remove_item(chat, id, &it)?);
            *budget -= 1;
        }
        Ok(())
    }
}
