//! A new member gets the group's recent messages as forwards in `=` batches, then its welcome message (Internal.hs `sendHistory`; Store/Messages.hs `getGroupHistoryItems`).

use crate::types::model::{ChatRef, MessageBody};

use super::fwd_batch::forward_batches;
use crate::chat::container::MsgContainer;
use crate::chat::limits::MAX_ENCODED_MSG_LENGTH;
use crate::chat::{ChatEvent, ChatMessage};
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::machine::Engine;
use crate::engine::records::{GroupRecord, ItemRecord, decode, keys};
use crate::limits::HISTORY_ITEMS;

/// Items read at a time while looking for them.
const PAGE: u32 = 200;

/// `include_in_history`: message content in the group's main chat, not deleted.
const fn in_history(it: &ItemRecord) -> bool {
    match it.body {
        MessageBody::Text(_)
        | MessageBody::Voice { .. }
        | MessageBody::Gif(_)
        | MessageBody::Image(_)
        | MessageBody::Video(_)
        | MessageBody::File(_) => true,
        MessageBody::Notice(_)
        | MessageBody::Call(_)
        | MessageBody::Stream(_)
        | MessageBody::Deleted => false,
    }
}

impl<A: AgentPort> Engine<A> {
    /// `exclude`: the user's items this member got another way (a send-status row).
    pub(crate) fn send_history(&mut self, group: i64, member: i64, exclude: &[i64]) -> Result<()> {
        let g = self.group(group)?;
        let conn = self
            .member(group, member)?
            .conn
            .ok_or_else(|| fail(Code::RequestFailed, "member connection not active"))?;
        let mut elements = Vec::new();
        for (id, it) in self.history_items(group, exclude)? {
            match self.history_elements(&g, id, &it) {
                Ok(els) => elements.extend(els),
                Err(e) => self
                    .log
                    .line(format_args!("sendHistory: an item left out: {}", e.detail)),
            }
        }
        elements.extend(self.welcome_element(&g)?);
        let (batches, dropped) = forward_batches(&elements);
        if dropped > 0 {
            self.log.line(format_args!(
                "sendHistory: dropped {dropped} oversized history messages"
            ));
        }
        for body in batches {
            self.agent
                .send(&mut self.store, &conn, self.state.pq_of(&conn), body)?;
        }
        Ok(())
    }

    /// The newest that go, oldest first.
    fn history_items(&mut self, group: i64, exclude: &[i64]) -> Result<Vec<(i64, ItemRecord)>> {
        let chat = keys::chat(ChatRef::Group(group));
        let mut out = Vec::new();
        let mut before = None;
        while out.len() < HISTORY_ITEMS {
            let page = self.store.page(&chat, before, PAGE)?;
            let Some(&(first, _)) = page.first() else {
                break;
            };
            let more = page.len() == PAGE as usize;
            before = Some(first);
            for (id, bytes) in page.into_iter().rev() {
                let it: ItemRecord = decode(&bytes)?;
                if in_history(&it) && !exclude.contains(&id) && out.len() < HISTORY_ITEMS {
                    out.push((id, it));
                }
            }
            if !more {
                break;
            }
        }
        out.sort_by_key(|(id, it)| (it.at, *id));
        Ok(out)
    }

    /// The group's description, sent last as the host's own message (`descrEvent_`).
    fn welcome_element(&mut self, g: &GroupRecord) -> Result<Option<Vec<u8>>> {
        let Some(descr) = &g.profile.description else {
            return Ok(None);
        };
        let json = ChatMessage::new(ChatEvent::MsgNew(MsgContainer::text(descr)))?.to_json()?;
        if json.len() > MAX_ENCODED_MSG_LENGTH {
            self.log
                .line(format_args!("sendHistory: welcome message too large"));
            return Ok(None);
        }
        Ok(Some(json))
    }
}
