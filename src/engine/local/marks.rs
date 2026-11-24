//! SetUnreadMark and SetAlias: a conversation's marks the user keeps alone (Commands.hs `APIChatUnread`, `APISetContactAlias`, `APISetGroupAlias`).

use crate::types::model::ChatRef;

use crate::engine::agent_port::AgentPort;
use crate::engine::error::Result;
use crate::engine::machine::Engine;
use crate::engine::records::ChatLocal;

impl<A: AgentPort> Engine<A> {
    /// Changes a conversation's local settings with `f`, and the chat list follows.
    pub(crate) fn change_local(
        &mut self,
        chat: ChatRef,
        f: impl FnOnce(&mut ChatLocal),
    ) -> Result<()> {
        match chat {
            ChatRef::Direct(id) => {
                let mut r = self.friend(id)?;
                f(&mut r.local);
                self.save_friend(r)?;
            }
            ChatRef::Group(id) => {
                let mut r = self.group(id)?;
                f(&mut r.local);
                self.save_group(r)?;
            }
        }
        self.state.chats_dirty = true;
        Ok(())
    }

    /// The conversation's local settings.
    pub(crate) fn local_of(&self, chat: ChatRef) -> Result<ChatLocal> {
        Ok(match chat {
            ChatRef::Direct(id) => self.friend(id)?.local,
            ChatRef::Group(id) => self.group(id)?.local,
        })
    }

    pub(crate) fn set_unread_mark(&mut self, chat: ChatRef, on: bool) -> Result<()> {
        self.change_local(chat, |l| l.marked_unread = on)
    }

    pub(crate) fn set_alias(&mut self, chat: ChatRef, alias: &str) -> Result<()> {
        let alias = alias.trim().to_owned();
        self.change_local(chat, |l| l.alias = alias)
    }
}
