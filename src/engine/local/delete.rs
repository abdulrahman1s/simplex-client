//! DeleteForMe: a message goes here only, with its file; nobody else is told (Commands.hs `APIDeleteChatItem` `CIDMInternal`).

use crate::types::Event;
use crate::types::model::{ChatRef, MessageAuthor, MessageBody};

use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::machine::Engine;

impl<A: AgentPort> Engine<A> {
    pub(crate) fn delete_for_me(&mut self, chat: ChatRef, item: i64) -> Result<()> {
        let it =
            (self.item(chat, item)?).ok_or_else(|| fail(Code::RequestFailed, "no such item"))?;
        let counted = it.author != MessageAuthor::Me
            && !matches!(it.body, MessageBody::Notice(_))
            && item > self.read_mark(chat);
        self.drop_item_file(chat, item)?;
        let m = self.remove_item(chat, item, &it)?;
        if counted {
            self.uncount(chat)?;
        }
        self.state.chats_dirty = true;
        self.emit(Event::Updated(vec![m]));
        Ok(())
    }

    /// One unread message fewer.
    fn uncount(&mut self, chat: ChatRef) -> Result<()> {
        match chat {
            ChatRef::Direct(id) => {
                let mut f = self.friend(id)?;
                f.unread = f.unread.saturating_sub(1);
                self.save_friend(f)?;
            }
            ChatRef::Group(id) => {
                let mut g = self.group(id)?;
                g.unread = g.unread.saturating_sub(1);
                self.save_group(g)?;
            }
        }
        Ok(())
    }
}
