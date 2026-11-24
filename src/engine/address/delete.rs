//! DeleteAddress: the queue goes, so the links stop working, and the requests waiting on it are forgotten (Commands.hs `APIDeleteMyAddress`).

use crate::types::Event;

use crate::engine::agent_port::AgentPort;
use crate::engine::error::Result;
use crate::engine::machine::Engine;
use crate::engine::records::{NS, keys};
use crate::store::Op;

impl<A: AgentPort> Engine<A> {
    pub(crate) fn delete_address(&mut self) -> Result<()> {
        let made = self.state.address.record.take().map(|a| a.conn);
        let pending = self.state.address.creating.take();
        for conn in made.iter().chain(pending.iter()) {
            self.drop_conn(conn)?;
        }
        let mut ops = vec![Op::Delete {
            ns: NS,
            key: keys::ADDRESS.to_vec(),
        }];
        // A group member's invitation did not come through the address, so it stays.
        let requests = &mut self.state.address.requests;
        let through: Vec<i64> = requests
            .values()
            .filter(|r| r.member.is_none())
            .map(|r| r.id)
            .collect();
        ops.extend(through.iter().map(|id| Op::Delete {
            ns: NS,
            key: keys::request(*id),
        }));
        self.store.apply(ops)?;
        requests.retain(|_, r| r.member.is_some());
        self.state.chats_dirty = true;
        self.emit(Event::Address(None));
        Ok(())
    }
}
