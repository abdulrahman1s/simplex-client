//! A message on one of our queues: the server layer, the per-queue box, then the envelope by kind; handshake and agent messages are acked here (Agent.hs `processSMPTransmissions`, `smpConfirmation`).
use crate::encoding::Decode;
use crate::smp::{RcvMsg, decrypt_message};
use crate::store::Store;

use super::seal::{HeaderKey, open};
use crate::agent::client::{AgentClient, AgentEvent, ConnId, OpKind};
use crate::agent::connection::ack_request;
use crate::agent::envelope::AgentMsgEnvelope;
use crate::agent::error::{AgentError, Result};

/// Whether the agent acks a message itself, or the engine will through `ack`.
pub(crate) enum Handled {
    Ack,
    Hold,
}

impl AgentClient {
    pub(crate) fn on_queue_msg(
        &mut self,
        store: &mut Store,
        id: ConnId,
        smp_msg_id: &[u8],
        body: &[u8],
    ) {
        match self.receive(store, id, smp_msg_id, body) {
            Ok(Handled::Hold) => {}
            Ok(Handled::Ack) => self.ack_queue(id, smp_msg_id),
            // Like the Haskell agent: report it, and ack so that it is not delivered forever.
            Err(err) => {
                self.emit(AgentEvent::Err { conn: id, err });
                self.ack_queue(id, smp_msg_id);
            }
        }
        self.kick(store, id);
    }

    fn receive(
        &mut self,
        store: &mut Store,
        id: ConnId,
        smp_msg_id: &[u8],
        body: &[u8],
    ) -> Result<Handled> {
        let rq = self
            .conn(id)?
            .rec
            .rcv
            .clone()
            .ok_or(AgentError::Prohibited("no queue"))?;
        let (ts, body) = match decrypt_message(&rq.server_secret()?, smp_msg_id, body)? {
            RcvMsg::Quota { .. } => {
                self.on_quota_notice(store, id)?;
                return Ok(Handled::Ack);
            }
            RcvMsg::Message { ts, body, .. } => (ts, body),
        };
        if let Some(h) = self.redelivered(store, id, smp_msg_id)? {
            return Ok(h);
        }
        let opened = open(&rq, &body)?;
        let env = AgentMsgEnvelope::from_bytes(&opened.envelope)?;
        match opened.header {
            HeaderKey::First(_) => self.on_handshake(store, id, &opened, env),
            HeaderKey::None => self.on_duplex(store, id, smp_msg_id, ts.0, env),
            // The server sends a confirmation again when our ACK was lost.
            HeaderKey::Repeated => match env {
                AgentMsgEnvelope::Confirmation { .. } | AgentMsgEnvelope::ContactRequest { .. } => {
                    Ok(Handled::Ack)
                }
                AgentMsgEnvelope::Message { .. }
                | AgentMsgEnvelope::Invitation { .. }
                | AgentMsgEnvelope::RatchetKey { .. } => {
                    Err(AgentError::Prohibited("msg: public header"))
                }
            },
        }
    }

    pub(crate) fn ack_queue(&mut self, id: ConnId, smp_msg_id: &[u8]) {
        let Some(rq) = self.conns.get(&id).and_then(|c| c.rec.rcv.clone()) else {
            return;
        };
        // An ACK that is lost makes the server send the message again; it is then recognised and acked.
        self.request(id, OpKind::Ack, &rq.server, ack_request(&rq, smp_msg_id));
    }
}
