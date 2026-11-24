//! MarkRead and MarkReadUpTo: the conversation is read up to its newest item or a given one (local; the codec reports nothing), which starts its timed items' count (Commands.hs `APIChatRead`, `APIChatItemsRead`).

use crate::types::model::{ChatRef, MessageAuthor, MessageBody};

use crate::engine::agent_port::AgentPort;
use crate::engine::error::Result;
use crate::engine::machine::Engine;
use crate::engine::records::{ItemRecord, decode, keys};

/// Items read per page while counting what is still unread.
const PAGE: u32 = 200;

impl<A: AgentPort> Engine<A> {
    pub(crate) fn mark_read(&mut self, chat: ChatRef) -> Result<()> {
        self.read_to(chat, None)
    }

    /// Read up to `upto`, or the newest item; a mark never moves back.
    pub(crate) fn read_to(&mut self, chat: ChatRef, upto: Option<i64>) -> Result<()> {
        let (read_up_to, unread) = match chat {
            ChatRef::Direct(id) => {
                let f = self.friend(id)?;
                (f.read_up_to, f.unread)
            }
            ChatRef::Group(id) => {
                let g = self.group(id)?;
                (g.read_up_to, g.unread)
            }
        };
        let newest = self.state.last.get(&chat).map_or(read_up_to, |m| m.id);
        let target = upto.map_or(newest, |u| u.min(newest));
        if target <= read_up_to && (unread == 0 || target < newest) {
            return Ok(());
        }
        let target = target.max(read_up_to);
        let left = if target >= newest {
            0
        } else {
            self.unread_after(chat, target)?
        };
        match chat {
            ChatRef::Direct(id) => {
                let mut f = self.friend(id)?;
                (f.read_up_to, f.unread) = (target, left);
                self.save_friend(f)?;
            }
            ChatRef::Group(id) => {
                let mut g = self.group(id)?;
                (g.read_up_to, g.unread) = (target, left);
                self.save_group(g)?;
            }
        }
        if let Some(last) = self.state.last.get_mut(&chat)
            && last.id <= target
        {
            last.unread = false;
        }
        self.start_read_timers(chat, target)
    }

    /// Received messages after `mark`, as the unread count shows them.
    fn unread_after(&mut self, chat: ChatRef, mark: i64) -> Result<u32> {
        let mut count = 0u32;
        let mut before = None;
        loop {
            let page = self.store.page(&keys::chat(chat), before, PAGE)?;
            let Some(&(oldest, _)) = page.first() else {
                return Ok(count);
            };
            for (id, bytes) in page.iter().rev() {
                if *id <= mark {
                    return Ok(count);
                }
                let it: ItemRecord = decode(bytes)?;
                let theirs = it.author != MessageAuthor::Me;
                if theirs && !matches!(it.body, MessageBody::Notice(_)) {
                    count = count.saturating_add(1);
                }
            }
            before = Some(oldest);
        }
    }
}
