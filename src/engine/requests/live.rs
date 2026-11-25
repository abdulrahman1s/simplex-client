//! Live messages: one the user writes as the friend or group watches, then finishes (Commands.hs `APISendMessages` live, `APIUpdateChatItem` live).

use crate::types::model::{ChatRef, MessageBody};

use crate::chat::ChatEvent;
use crate::chat::container::MsgMentions;
use crate::chat::content::MsgContent;
use crate::chat::ids::SharedMsgId;
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::machine::Engine;
use crate::engine::timed::rules::deadline;
use crate::limits::{EDIT_WINDOW_MS, ROLE_TO_SEND};

impl<A: AgentPort> Engine<A> {
    /// The text so far goes as an update, still live; `done` finishes it, and a disappearing
    /// message's count starts then.
    pub(crate) fn update_live(
        &mut self,
        chat: ChatRef,
        item: i64,
        text: &str,
        done: bool,
    ) -> Result<()> {
        self.may_change(chat, &ROLE_TO_SEND)?;
        let (mut it, shared) = self.own_item(chat, item, EDIT_WINDOW_MS)?;
        if !it.live {
            return Err(fail(Code::RequestFailed, "not a live message"));
        }
        let MessageBody::Text(_) = it.body else {
            return Err(fail(Code::RequestFailed, "not a live message"));
        };
        let in_group = matches!(chat, ChatRef::Group(_));
        self.send_to_chat(
            chat,
            ChatEvent::MsgUpdate {
                msg_id: SharedMsgId(shared),
                content: MsgContent::Text {
                    text: text.to_owned(),
                },
                mentions: MsgMentions::new(),
                ttl: it.timed.map(|t| t.ttl),
                live: (!done).then_some(true),
                scope: None,
                as_group: in_group.then_some(false),
            },
        )?;
        it.body = MessageBody::Text(text.to_owned());
        it.live = !done;
        if let Some(t) = it.timed.as_mut().filter(|t| done && t.delete_at.is_none()) {
            let at = deadline(self.clock.wall_ms(), t.ttl);
            t.delete_at = Some(at);
            self.schedule(chat, item, at)?;
        }
        self.save_changed(chat, item, &it)
    }
}
