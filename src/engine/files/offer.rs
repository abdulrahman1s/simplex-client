//! Whether a file may go and to whom its offer goes: the user's own file, a connected friend, or a group that allows files (Commands.hs `checkSndFile`; Internal.hs `prohibitedGroupContent` when sending).

use crate::types::model::ChatRef;

use super::send::PQ;
use crate::chat::{ChatMessage, encode_chat_message};
use crate::engine::agent_port::{AgentMsgId, AgentPort, ConnId};
use crate::engine::error::{Code, Result, fail};
use crate::engine::groups::policy;
use crate::engine::machine::Engine;
use crate::limits::ROLE_TO_SEND;

impl<A: AgentPort> Engine<A> {
    /// A friend must be connected; in a group the user must be an author and files allowed
    /// for their role (Internal.hs `prohibitedGroupContent` when sending).
    pub(super) fn may_send_file(&self, chat: ChatRef) -> Result<()> {
        match chat {
            ChatRef::Direct(friend) => {
                if !self.friend(friend)?.reachable() {
                    return Err(fail(Code::RequestFailed, "not connected"));
                }
            }
            ChatRef::Group(group) => {
                let g = self.group(group)?;
                Self::assert_my_role(&g, &ROLE_TO_SEND)?;
                if !policy::allowed(&g, policy::Feature::Files, &g.my_role) {
                    return Err(fail(Code::NotAllowed, "feature not allowed: files"));
                }
            }
        }
        Ok(())
    }

    /// The message with the file's invitation: to the friend, or to each current member (kept
    /// for those still connecting). What was sent now: (member, connection, agent message id);
    /// the member is 0 for a friend. Only those get a description (`saveMemberFD`).
    pub(super) fn send_offer(
        &mut self,
        chat: ChatRef,
        msg: &ChatMessage,
    ) -> Result<Vec<(i64, ConnId, AgentMsgId)>> {
        match chat {
            ChatRef::Direct(friend) => {
                let conn = self.friend(friend)?.conn;
                let body = encode_chat_message(msg, PQ)?;
                let id = self
                    .agent
                    .send(&mut self.store, &conn, self.state.pq_of(&conn), body)?;
                Ok(vec![(0, conn, id)])
            }
            ChatRef::Group(group) => self.send_to_group(group, msg, None),
        }
    }
}
