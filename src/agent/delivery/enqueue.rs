//! Queue a message: ratchet-encrypt it as the next in our chain, seal it, and store body and ratchet together before the first attempt (Agent.hs `sendMessage'`, `enqueueMessageB`).
use crate::encoding::{Encode, Version};
use crate::ratchet::{PqEncryption, Ratchet, e2e_enc_agent_msg_length};
use crate::store::Store;

use super::seal::seal;
use crate::agent::client::{AgentClient, AgentMsgId, ConnId};
use crate::agent::envelope::AgentMsgEnvelope;
use crate::agent::error::{AgentError, Result};
use crate::agent::message::{AMessage, APrivHeader, AgentMessage};
use crate::agent::msg_hash;
use crate::agent::records::{Batch, ConnRecord, ConnStatus, PendingSend, SendKind, SentMsg};

impl AgentClient {
    /// `body` is chat's message bytes; `pq` turns the KEM on or off from this message (chat
    /// passes its connection's `pqEncryption`). `Sent` follows when the server takes it, or `Merr`.
    pub fn send(
        &mut self,
        store: &mut Store,
        conn: ConnId,
        pq: PqEncryption,
        body: Vec<u8>,
    ) -> Result<AgentMsgId> {
        if self.conn(conn)?.rec.status != ConnStatus::Active {
            return Err(AgentError::Prohibited("send before CON"));
        }
        self.enqueue_message(store, conn, AMessage::AMsg(body), SendKind::Msg, Some(pq))
    }

    pub(crate) fn enqueue_message(
        &mut self,
        store: &mut Store,
        id: ConnId,
        msg: AMessage,
        kind: SendKind,
        pq: Option<PqEncryption>,
    ) -> Result<AgentMsgId> {
        let (mut rec, ratchet) = self.editable(id)?;
        let mut ratchet = ratchet.ok_or(AgentError::Prohibited("no ratchet"))?;
        rec.ensure_can_send()?;
        // `sendMessagesB_`: PQ support, once asked for, stays on.
        if pq == Some(PqEncryption::ON) {
            rec.pq = true;
        }
        let sq = rec
            .snd
            .clone()
            .ok_or(AgentError::Prohibited("no send queue"))?;
        let v = self.envelope_agent_version();
        let body = seal(
            &sq,
            &agent_envelope(&mut rec, &mut ratchet, msg, v, pq)?,
            false,
        )?;
        let snd_msg_id = rec.chain.snd_id;
        let mut b = Batch::default();
        if kind == SendKind::Msg {
            let sent = SentMsg {
                msg_id: AgentMsgId(rec.last_id + 1),
                hash: rec.chain.snd_hash.clone(),
            };
            b.sent(id, snd_msg_id, &sent)?;
        }
        self.enqueue_built(store, rec, ratchet, kind, body, b)
    }

    /// Stores a built SEND body with the ratchet step that made it, then starts sending.
    pub(crate) fn enqueue_built(
        &mut self,
        store: &mut Store,
        mut rec: ConnRecord,
        ratchet: Ratchet,
        kind: SendKind,
        body: Vec<u8>,
        mut b: Batch,
    ) -> Result<AgentMsgId> {
        rec.last_id += 1;
        let (conn, id) = (rec.id, AgentMsgId(rec.last_id));
        let created_at = self.now();
        b.pending(
            conn,
            &PendingSend {
                id,
                kind,
                body,
                created_at,
            },
        )?;
        self.save(store, rec, Some(ratchet), b)?;
        self.conn_mut(conn)?.sends.push_back(id);
        self.kick(store, conn);
        Ok(id)
    }
}

/// The next agent message of our chain as an envelope, before the per-queue box; moves the chain on.
/// `pq`: `Some` for chat's messages, `None` (keep the KEM as it is) for the agent's own.
pub(crate) fn agent_envelope(
    rec: &mut ConnRecord,
    ratchet: &mut Ratchet,
    msg: AMessage,
    agent_version: Version,
    pq: Option<PqEncryption>,
) -> Result<Vec<u8>> {
    let snd_msg_id = rec.chain.snd_id + 1;
    let header = APrivHeader {
        snd_msg_id,
        prev_msg_hash: rec.chain.snd_hash.clone(),
    };
    let plain = AgentMessage::Message {
        header,
        message: msg,
    }
    .to_bytes()?;
    // The body is padded for the support the header is about to have (`rcEncryptHeader`).
    let enable = pq.unwrap_or(ratchet.pq_enabled());
    let len = e2e_enc_agent_msg_length(ratchet.pq_support().enable_with(enable));
    let env = AgentMsgEnvelope::Message {
        agent_version,
        enc_agent_message: ratchet.encrypt(&plain, len, pq)?,
    };
    rec.chain.snd_id = snd_msg_id;
    rec.chain.snd_hash = msg_hash(&plain).to_vec();
    Ok(env.to_bytes()?)
}
