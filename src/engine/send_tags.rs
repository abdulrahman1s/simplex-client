//! The messages a tagged request sent, and their `Sent` or failure told against its tag.

use crate::types::Event;
use crate::types::model::SendStatus;

use crate::engine::agent_port::{AgentFailure, AgentMsgId, AgentPort, ConnId};
use crate::engine::machine::Engine;

impl<A: AgentPort> Engine<A> {
    /// `agent.send`, keeping the tag of the request being handled with the message.
    pub(crate) fn agent_send(
        &mut self,
        conn: &ConnId,
        pq: bool,
        body: Vec<u8>,
    ) -> Result<AgentMsgId, AgentFailure> {
        let id = self.agent.send(&mut self.store, conn, pq, body)?;
        if let Some(tag) = self.tagging {
            self.send_tags.insert((*conn, id), tag);
        }
        Ok(id)
    }

    /// The tag's status for one message; its entry goes with it.
    pub(crate) fn tag_outcome(&mut self, conn: &ConnId, msg: AgentMsgId, status: SendStatus) {
        let Some(tag) = self.send_tags.remove(&(*conn, msg)) else {
            return;
        };
        let member = self.state.member_of(conn).map(|(_, m)| m);
        self.emit(Event::SendStatus {
            tag,
            member,
            status,
        });
    }
}
