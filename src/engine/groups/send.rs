//! Sending to a group's members: now to those whose connection works, kept for those still connecting (Internal.hs `sendGroupMessage`, `sendGroupMemberMessage`, `sendDirectMemberMessage`; group connections are PQ off).

use super::send_action::{SendAction, send_action};
use crate::chat::{ChatMessage, encode_chat_message};
use crate::engine::agent_port::{AgentMsgId, AgentPort, ConnId};
use crate::engine::error::{Code, Result, fail};
use crate::engine::machine::Engine;

impl<A: AgentPort> Engine<A> {
    /// `sendDirectMemberMessage`: only on a connection that works now.
    pub(crate) fn send_to_member(
        &mut self,
        group: i64,
        member: i64,
        msg: &ChatMessage,
    ) -> Result<(ConnId, AgentMsgId)> {
        let conn = self
            .member(group, member)?
            .reachable()
            .ok_or_else(|| fail(Code::RequestFailed, "member not connected"))?;
        let body = encode_chat_message(msg, false)?;
        Ok((conn, self.agent_send(&conn, self.state.pq_of(&conn), body)?))
    }

    /// To every current member but `except` (`getGroupRecipients`).
    pub(crate) fn send_to_group(
        &mut self,
        group: i64,
        msg: &ChatMessage,
        except: Option<i64>,
    ) -> Result<Vec<(i64, ConnId, AgentMsgId)>> {
        if self
            .state
            .groups
            .get(&group)
            .is_some_and(|g| g.channel.is_some())
        {
            return self.send_to_channel(group, msg, false);
        }
        let to: Vec<i64> = self
            .state
            .members_of(group)
            .filter(|m| Some(m.id) != except && m.status.current())
            .map(|m| m.id)
            .collect();
        self.send_to_members(group, &to, msg)
    }

    /// Each recipient as `memberSendAction` says; a send or a save that fails is logged and
    /// that member left out. What was sent now: (member, connection, agent message id).
    pub(crate) fn send_to_members(
        &mut self,
        group: i64,
        to: &[i64],
        msg: &ChatMessage,
    ) -> Result<Vec<(i64, ConnId, AgentMsgId)>> {
        let g = self.group(group)?;
        let body = encode_chat_message(msg, false)?;
        let shared = msg.msg_id.as_ref().map(|m| m.0.as_slice());
        let mut sent = Vec::with_capacity(to.len());
        for &member in to {
            let Some(m) = self.state.members.get(&(group, member)) else {
                continue;
            };
            let done = match send_action(&g, m, to, &msg.event) {
                Some(SendAction::Send(conn)) => self
                    .agent_send(&conn, self.state.pq_of(&conn), body.clone())
                    .map(|id| sent.push((member, conn, id)))
                    .map_err(Into::into),
                Some(SendAction::Pending) => self.queue_pending(group, member, &body, shared),
                Some(SendAction::Forwarded) | None => Ok(()),
            };
            if let Err(e) = done {
                self.log
                    .line(format_args!("a group message was not sent: {}", e.detail));
            }
        }
        Ok(sent)
    }
}
