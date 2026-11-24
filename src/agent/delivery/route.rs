//! An opened envelope by the key that opened it: handshakes under the sender's header key, ratchet traffic under the stored secret (Agent.hs `processClientMsg`).
use crate::store::Store;

use super::receive::Handled;
use super::seal::Opened;
use crate::agent::client::{AgentClient, ConnId};
use crate::agent::connection::Confirmation;
use crate::agent::envelope::AgentMsgEnvelope;
use crate::agent::error::{AgentError, Result};

impl AgentClient {
    /// The sender's key opened it: only a handshake may come this way.
    pub(super) fn on_handshake(
        &mut self,
        store: &mut Store,
        id: ConnId,
        opened: &Opened,
        env: AgentMsgEnvelope,
    ) -> Result<Handled> {
        match env {
            AgentMsgEnvelope::Confirmation {
                agent_version,
                e2e_encryption,
                enc_conn_info,
            } => {
                let conf = Confirmation {
                    agent_version,
                    e2e: e2e_encryption,
                    enc_conn_info,
                };
                self.on_confirmation(store, id, opened, conf)?;
                Ok(Handled::Ack)
            }
            AgentMsgEnvelope::Invitation {
                conn_req,
                conn_info,
                ..
            } => {
                self.on_contact_request(store, id, &conn_req, conn_info)?;
                Ok(Handled::Ack)
            }
            // Our addresses publish no ratchet keys, so no request can name one (`unknown ratchetKeyId`).
            AgentMsgEnvelope::ContactRequest { .. } => {
                Err(AgentError::Prohibited("request with unknown ratchet keys"))
            }
            AgentMsgEnvelope::Message { .. } | AgentMsgEnvelope::RatchetKey { .. } => {
                Err(AgentError::Prohibited("handshake: incorrect state"))
            }
        }
    }

    /// The stored secret opened it: a ratchet message or a new ratchet key.
    pub(super) fn on_duplex(
        &mut self,
        store: &mut Store,
        id: ConnId,
        smp_msg_id: &[u8],
        ts: i64,
        env: AgentMsgEnvelope,
    ) -> Result<Handled> {
        match env {
            AgentMsgEnvelope::Message {
                agent_version,
                enc_agent_message,
            } => {
                self.update_conn_version(store, id, agent_version)?;
                self.on_agent_message(store, id, smp_msg_id, ts, &enc_agent_message)
            }
            AgentMsgEnvelope::RatchetKey {
                agent_version,
                e2e_encryption,
                info,
            } => {
                self.update_conn_version(store, id, agent_version)?;
                self.on_ratchet_key(store, id, e2e_encryption, info)
            }
            AgentMsgEnvelope::Confirmation { .. }
            | AgentMsgEnvelope::Invitation { .. }
            | AgentMsgEnvelope::ContactRequest { .. } => {
                Err(AgentError::Prohibited("msg: bad client msg"))
            }
        }
    }
}
