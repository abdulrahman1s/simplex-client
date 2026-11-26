//! An owner posts as the channel: to every connected relay, `asGroup` set, signed when asked; a signed body that would not fit is refused before anything is sent (Commands.hs `/_send` with `asGroup`, `sign=on`; Internal.hs `groupMsgSigning`, `createSndMessages`).

use crate::chat::element::encode_signed_element;
use crate::chat::{ChatEvent, ChatMessage, encode_chat_message};
use crate::engine::agent_port::{AgentMsgId, AgentPort, ConnId};
use crate::engine::error::{Code, Result, fail};
use crate::engine::machine::Engine;
use crate::limits::MAX_ENCODED_MSG_LENGTH;

use super::keys::{binding, sign};

impl<A: AgentPort> Engine<A> {
    /// What `send_to_group` sends in a channel: the owner's content as the channel's, to its
    /// relays only. Subscribers are reached by the relays, never directly.
    pub(crate) fn send_to_channel(
        &mut self,
        group: i64,
        msg: &ChatMessage,
        signed: bool,
    ) -> Result<Vec<(i64, ConnId, AgentMsgId)>> {
        self.send_to_relays_of(group, msg, signed, None)
    }

    /// As `send_to_channel`, to the relays in `to` only.
    pub(super) fn send_to_relays(
        &mut self,
        group: i64,
        msg: &ChatMessage,
        signed: bool,
        to: &[i64],
    ) -> Result<Vec<(i64, ConnId, AgentMsgId)>> {
        if to.is_empty() {
            return Ok(Vec::new());
        }
        self.send_to_relays_of(group, msg, signed, Some(to))
    }

    fn send_to_relays_of(
        &mut self,
        group: i64,
        msg: &ChatMessage,
        signed: bool,
        only: Option<&[i64]>,
    ) -> Result<Vec<(i64, ConnId, AgentMsgId)>> {
        let g = self.group(group)?;
        let c = g
            .channel
            .clone()
            .ok_or_else(|| fail(Code::RequestFailed, "not a channel"))?;
        let mut msg = msg.clone();
        if c.owner() {
            if let ChatEvent::MsgNew(m) = &mut msg.event {
                m.as_group = Some(true);
            }
            if let ChatEvent::MsgUpdate { as_group, .. } = &mut msg.event {
                *as_group = Some(true);
            }
        }
        let body = if signed || super::verify::requires_signature(&msg.event) {
            let s = sign(
                &c.member_private,
                &binding(&c.public_group_id, &g.my_member_id)?,
                msg.to_json()?,
            )?;
            let element = encode_signed_element(&s)?;
            // 7.1.4 reports a signed post this large sent and never sends it; refuse it here.
            if element.len() > MAX_ENCODED_MSG_LENGTH {
                return Err(fail(Code::TextTooLong, "signed message too large"));
            }
            element
        } else {
            encode_chat_message(&msg, false)?
        };
        let relays: Vec<(i64, ConnId)> = (self.state.members_of(group))
            .filter(|m| m.role == crate::chat::group::GroupMemberRole::Relay)
            .filter(|m| only.is_none_or(|o| o.contains(&m.id)))
            .filter_map(|m| m.reachable().map(|conn| (m.id, conn)))
            .collect();
        if relays.is_empty() {
            return Err(fail(Code::NoConnection, "no channel relay is connected"));
        }
        let mut sent = Vec::with_capacity(relays.len());
        for (member, conn) in relays {
            // Group connections are PQ off.
            match self.agent_send(&conn, false, body.clone()) {
                Ok(id) => sent.push((member, conn, id)),
                Err(e) => self
                    .log
                    .line(format_args!("a channel post was not sent: {}", e.detail())),
            }
        }
        Ok(sent)
    }
}
