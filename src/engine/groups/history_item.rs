//! One item as a group's history sends it: its content again, unsigned, forwarded from its author at its own time, with its file's description (Internal.hs `sendHistory` `itemForwardMsgs`, `processContentItem`).

use crate::types::model::{ChatRef, MessageBody};

use super::fwd_batch::fwd_member_name;
use super::mention::quote_mentions;
use crate::chat::container::{MsgContainer, MsgMention, MsgMentions};
use crate::chat::content::MsgContent;
use crate::chat::element::encode_forward_element;
use crate::chat::forward::{FwdSender, GrpMsgForward};
use crate::chat::ids::SharedMsgId;
use crate::chat::version::{CHAT_INITIAL_VRANGE, SUPPORTED_CHAT_VRANGE};
use crate::chat::{ChatEvent, ChatMessage};
use crate::engine::agent_port::AgentPort;
use crate::engine::error::Result;
use crate::engine::files::{HistoryFile, descr_parts};
use crate::engine::machine::Engine;
use crate::engine::records::{GroupRecord, ItemRecord};
use crate::engine::time::iso;

impl<A: AgentPort> Engine<A> {
    /// The `>` elements of item `id`: none for one with no text and no file to fetch.
    pub(crate) fn history_elements(
        &mut self,
        g: &GroupRecord,
        id: i64,
        it: &ItemRecord,
    ) -> Result<Vec<Vec<u8>>> {
        let mentions = self.history_mentions(g, it);
        let (content, file) = match &it.body {
            MessageBody::Text(t) => {
                let text = quote_mentions(t, mentions.keys().map(String::as_str));
                (MsgContent::Text { text }, None)
            }
            _ => match self.history_file(ChatRef::Group(g.id), id, it)? {
                Some(found) => found,
                None => return Ok(Vec::new()),
            },
        };
        // `msgContentHasText`
        if file.is_none() && content.text().is_empty() {
            return Ok(Vec::new());
        }
        let (author, name, v) = match it.member {
            None => (
                g.my_member_id.clone(),
                self.state.user.name.clone(),
                SUPPORTED_CHAT_VRANGE,
            ),
            Some(m) => {
                let m = self.member(g.id, m)?;
                let v = m.chat_v.unwrap_or(CHAT_INITIAL_VRANGE);
                (m.member_id, m.profile.display_name, v)
            }
        };
        let fwd = GrpMsgForward {
            sender: FwdSender::Member {
                member_id: author,
                member_name: fwd_member_name(&name),
            },
            broker_ts: iso(it.at),
        };
        let mut c = match it.reply.as_ref().and_then(|q| q.id) {
            Some(quoted) => MsgContainer::quote(self.group_quote(g, quoted)?.0, content),
            None => MsgContainer::simple(content),
        };
        c.mentions = mentions;
        c.file = file.as_ref().map(|f| f.invitation.clone());
        let shared = it.shared_id.clone();
        let mut events = vec![(shared.clone().map(SharedMsgId), ChatEvent::MsgNew(c))];
        if let (Some(HistoryFile { descr, .. }), Some(s)) = (&file, &shared) {
            events.extend(descr_parts(s, descr, None).into_iter().map(|e| (None, e)));
        }
        events
            .into_iter()
            .map(|(msg_id, event)| {
                let json = ChatMessage {
                    v: Some(v),
                    msg_id,
                    event,
                }
                .to_json()?;
                Ok(encode_forward_element(&fwd, &json)?)
            })
            .collect()
    }

    /// The item's mentions by member id; a member no longer known is left out.
    fn history_mentions(&self, g: &GroupRecord, it: &ItemRecord) -> MsgMentions {
        it.mentions
            .iter()
            .filter_map(|m| {
                let member_id = self.member_id_of(g, m.member).ok()?;
                Some((m.name.clone(), MsgMention { member_id }))
            })
            .collect()
    }
}
