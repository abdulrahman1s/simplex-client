//! Delete: the user deletes a message for everyone, and how a deleted item goes (Commands.hs
//! `APIDeleteChatItem` broadcast; Subscriber.hs `deleteGroupCIs`, `markGroupCIsDeleted`).

use crate::types::Event;
use crate::types::model::{ChatFeature, ChatMessage, ChatRef, MessageBody};

use crate::chat::ChatEvent;
use crate::chat::ids::SharedMsgId;
use crate::engine::agent_port::AgentPort;
use crate::engine::error::Result;
use crate::engine::groups::policy;
use crate::engine::machine::Engine;
use crate::engine::records::{ItemRecord, NS, keys};
use crate::limits::{DELETE_WINDOW_MS, ROLE_TO_DELETE_OWN};

impl<A: AgentPort> Engine<A> {
    /// Commands.hs `APIDeleteChatItem` broadcast: the item goes for good when the preferences
    /// allow full deletion for the user, else it stays, marked deleted.
    pub(crate) fn delete(&mut self, chat: ChatRef, item: i64) -> Result<()> {
        self.may_change(chat, &ROLE_TO_DELETE_OWN)?;
        let (it, shared) = self.own_item(chat, item, DELETE_WINDOW_MS)?;
        self.send_to_chat(
            chat,
            ChatEvent::MsgDel {
                msg_id: SharedMsgId(shared),
                member_id: None,
                scope: None,
                only_history: false,
            },
        )?;
        let full = match chat {
            ChatRef::Direct(friend) => {
                self.enabled(&self.friend(friend)?, ChatFeature::FullDelete)
                    .for_user
            }
            ChatRef::Group(group) => {
                let g = self.group(group)?;
                policy::allowed(&g, policy::Feature::FullDelete, &g.my_role)
            }
        };
        self.finish_delete_in(chat, item, it, full)
    }

    pub(crate) fn finish_delete(
        &mut self,
        friend: i64,
        item: i64,
        it: ItemRecord,
        full: bool,
    ) -> Result<()> {
        self.finish_delete_in(ChatRef::Direct(friend), item, it, full)
    }

    /// Marks the item deleted, or removes it from the store; the page is told it is deleted either way.
    pub(crate) fn finish_delete_in(
        &mut self,
        chat: ChatRef,
        item: i64,
        mut it: ItemRecord,
        full: bool,
    ) -> Result<()> {
        if !full {
            it.body = MessageBody::Deleted;
            it.reactions.clear();
            return self.save_changed(chat, item, &it);
        }
        let m = self.remove_item(chat, item, &it)?;
        self.emit(Event::Updated(vec![m]));
        Ok(())
    }

    /// The item leaves the store with everything kept about it; the page is shown it deleted.
    pub(crate) fn remove_item(
        &mut self,
        chat: ChatRef,
        item: i64,
        it: &ItemRecord,
    ) -> Result<ChatMessage> {
        self.store.delete_item(&keys::chat(chat), item)?;
        if let Some(shared) = &it.shared_id {
            self.store
                .delete(NS, &keys::shared(chat, it.member, shared))?;
        }
        // Finding the members' emoji scans the whole namespace, so only an item that counts
        // some looks; an emptied record left behind goes with its group.
        if !it.reactions.is_empty() {
            self.forget_group_reactions(chat, item)?;
        }
        self.leave_timed(chat, item, it.timed)?;
        let mut gone = it.clone();
        gone.body = MessageBody::Deleted;
        gone.reactions.clear();
        let m = gone.to_wire(item, chat, self.read_mark(chat));
        Ok(self.shown_last(chat, &gone, m))
    }
}
