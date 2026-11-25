//! A friend's negotiated post-quantum state, kept after each message as the agent reports it (Internal.hs `createContactPQSndItem`, `updateContactPQRcv`; Types.hs `connPQEnabled`).

use crate::engine::agent_port::{AgentPort, ConnId};
use crate::engine::machine::Engine;

impl<A: AgentPort> Engine<A> {
    /// The chat list goes out again when it changed.
    pub(crate) fn note_pq(&mut self, conn: &ConnId) {
        let Some(id) = self.state.friend_of(conn) else {
            return;
        };
        let on = self.agent.pq_on(conn);
        let Some(mut f) = self.state.friends.get(&id).cloned() else {
            return;
        };
        if f.pq_on == on {
            return;
        }
        f.pq_on = on;
        match self.save_friend(f) {
            Ok(()) => self.state.chats_dirty = true,
            Err(e) => self
                .log
                .line(format_args!("the PQ state was not kept: {e}")),
        }
    }
}
