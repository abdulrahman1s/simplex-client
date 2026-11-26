//! SendData: application data as `x.msg.new` with a content type the apps do not know, kept nowhere (Protocol.hs `MCUnknown`).

use serde_json::{Map, Value};

use crate::types::model::ChatRef;

use crate::chat::container::MsgContainer;
use crate::chat::{ChatEvent, ChatMessage, app_data};
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::machine::Engine;
use crate::engine::requests::sized;
use crate::limits::ROLE_TO_SEND;

use super::invite::PQ;
use super::size::encode_sized;

impl<A: AgentPort> Engine<A> {
    /// The chat's timer goes with it, so the fallback an official app shows goes when the
    /// chat's messages do.
    pub(crate) fn send_data(
        &mut self,
        chat: ChatRef,
        tag: &str,
        json: &Map<String, Value>,
        fallback: &str,
    ) -> Result<()> {
        self.send_data_as(chat, tag, json, fallback, false)
    }

    /// `signed`: in a channel, signed with the user's member key (`sign=on`).
    pub(crate) fn send_data_as(
        &mut self,
        chat: ChatRef,
        tag: &str,
        json: &Map<String, Value>,
        fallback: &str,
        signed: bool,
    ) -> Result<()> {
        if let Some(why) = app_data::refusal(tag, json) {
            return Err(fail(Code::RequestFailed, why));
        }
        let mut container = MsgContainer::simple(app_data::content(tag, json, fallback));
        container.ttl = self.outgoing_timed(chat)?.map(|t| t.ttl);
        let msg = ChatMessage::new(ChatEvent::MsgNew(container))?;
        match chat {
            ChatRef::Direct(id) => {
                let f = self.friend(id)?;
                if !f.reachable() {
                    return Err(fail(Code::RequestFailed, "not connected"));
                }
                let body = encode_sized(&msg, PQ)?;
                self.agent_send(&f.conn, f.pq, body)?;
            }
            ChatRef::Group(group) => {
                let g = self.group(group)?;
                Self::assert_my_role(&g, &ROLE_TO_SEND)?;
                sized(&msg)?;
                if g.channel.is_some() {
                    self.send_to_channel(group, &msg, signed)?;
                } else {
                    self.send_to_group(group, &msg, None)?;
                }
            }
        }
        Ok(())
    }
}
