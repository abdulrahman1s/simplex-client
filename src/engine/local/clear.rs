//! ClearChat: every message of a conversation goes here, with its files (Commands.hs `APIClearChat`).

use crate::types::Event;
use crate::types::model::ChatRef;

use crate::engine::agent_port::AgentPort;
use crate::engine::error::Result;
use crate::engine::machine::Engine;
use crate::engine::records::{ItemRecord, decode, keys};

/// Items read per page.
const PAGE: u32 = 500;

impl<A: AgentPort> Engine<A> {
    /// The conversation stays, empty and read; the page is told it was cleared.
    pub(crate) fn clear_chat(&mut self, chat: ChatRef) -> Result<()> {
        self.local_of(chat)?;
        loop {
            let page = self.store.page(&keys::chat(chat), None, PAGE)?;
            if page.is_empty() {
                break;
            }
            for (id, bytes) in page {
                let it: ItemRecord = decode(&bytes)?;
                self.drop_item_file(chat, id)?;
                self.remove_item(chat, id, &it)?;
            }
        }
        self.forget_timed(chat)?;
        self.state.last.remove(&chat);
        match chat {
            ChatRef::Direct(id) => {
                let mut f = self.friend(id)?;
                f.unread = 0;
                self.save_friend(f)?;
            }
            ChatRef::Group(id) => {
                let mut g = self.group(id)?;
                g.unread = 0;
                self.save_group(g)?;
            }
        }
        self.state.chats_dirty = true;
        self.emit(Event::Cleared(chat));
        Ok(())
    }
}
