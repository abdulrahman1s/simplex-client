//! A message that did not follow the one before it: a notice in its chat (Subscriber.hs `checkIntegrityCreateItem`).

use crate::agent::{MsgErrorType, MsgIntegrity};
use crate::types::model::{ChatNotice, IntegrityError};

use crate::engine::agent_port::{AgentPort, ConnId};
use crate::engine::error::Result;
use crate::engine::machine::Engine;

fn shown(e: MsgErrorType) -> IntegrityError {
    match e {
        MsgErrorType::Skipped { from, to } => IntegrityError::Skipped { from, to },
        MsgErrorType::BadId(id) => IntegrityError::BadId(id),
        MsgErrorType::BadHash => IntegrityError::BadHash,
        MsgErrorType::Duplicate => IntegrityError::Duplicate,
    }
}

impl<A: AgentPort> Engine<A> {
    /// Told before the message itself is handled, as `withAckMessage` does; the message is still handled.
    pub(crate) fn on_integrity(&mut self, conn: &ConnId, integrity: MsgIntegrity) -> Result<()> {
        let MsgIntegrity::Error(e) = integrity else {
            return Ok(());
        };
        let notice = ChatNotice::Integrity(shown(e));
        if let Some(friend) = self.state.friend_of(conn) {
            return self.notice(friend, notice);
        }
        if let Some((group, member)) = self.state.member_of(conn) {
            return self.group_notice(group, Some(member), notice);
        }
        Ok(())
    }
}
