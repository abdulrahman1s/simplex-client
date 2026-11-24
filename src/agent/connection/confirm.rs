//! A confirmation on our queue: the joiner's opens the inviter's ratchet and brings `Conf`; the inviter's brings `Info` and `Con` to the joiner (Agent.hs `smpConfirmation`; HANDSHAKE.md §3, §5).
use crate::crypto::PublicKey;
use crate::encoding::{Decode, Version};
use crate::ratchet::{PqSupport, Ratchet, RatchetVersions, pq_x3dh_rcv, supported_e2e_range};
use crate::store::Store;

use super::params::from_wire;
use crate::agent::client::{AgentClient, AgentEvent, ConnId};
use crate::agent::delivery::{HeaderKey, Opened};
use crate::agent::e2e::E2eParams as WireParams;
use crate::agent::error::{AgentError, Result};
use crate::agent::message::AgentMessage;
use crate::agent::msg_hash;
use crate::agent::records::{Batch, ConnRecord, ConnStatus};
use crate::agent::version::{MIN_SUPPORTED_AGENT, min_client_version};

/// The fields of `AgentConfirmation`.
pub(crate) struct Confirmation {
    pub agent_version: Version,
    pub e2e: Option<WireParams>,
    pub enc_conn_info: Vec<u8>,
}

impl AgentClient {
    pub(crate) fn on_confirmation(
        &mut self,
        store: &mut Store,
        id: ConnId,
        opened: &Opened,
        conf: Confirmation,
    ) -> Result<()> {
        let (rec, ratchet) = self.editable(id)?;
        let HeaderKey::First(sender) = opened.header.clone() else {
            return Err(AgentError::Prohibited("no e2e key"));
        };
        match rec.status {
            ConnStatus::Invited => {
                self.joiner_confirmed(store, rec, ratchet, opened.version, sender, conf)
            }
            ConnStatus::Joined => {
                let ratchet = ratchet.ok_or(AgentError::Prohibited("no ratchet"))?;
                self.inviter_confirmed(store, rec, ratchet, opened.version, sender, conf)
            }
            ConnStatus::Creating
            | ConnStatus::Addressing
            | ConnStatus::Address
            | ConnStatus::Applying
            | ConnStatus::Requesting
            | ConnStatus::Confirmed
            | ConnStatus::Allowing
            | ConnStatus::Accepting
            | ConnStatus::Resolving
            | ConnStatus::Securing
            | ConnStatus::Replying
            | ConnStatus::Active => Err(AgentError::Prohibited("confirmation in this state")),
        }
    }

    /// The inviter's side: the joiner's ratchet params start ours, and its reply queue waits for `allow`.
    fn joiner_confirmed(
        &mut self,
        store: &mut Store,
        mut rec: ConnRecord,
        request_ratchet: Option<Ratchet>,
        v: Version,
        sender: PublicKey,
        conf: Confirmation,
    ) -> Result<()> {
        if rec.x3dh.is_none() {
            return self.answer_confirmed(store, rec, request_ratchet, v, sender, conf);
        }
        let wire = conf.e2e.ok_or(AgentError::Prohibited(
            "confirmation without ratchet params",
        ))?;
        let snd = from_wire(&wire)?;
        if !supported_e2e_range().contains(snd.version) {
            return Err(AgentError::Version);
        }
        let x3dh = rec
            .x3dh
            .take()
            .ok_or(AgentError::Prohibited("no X3DH keys"))?;
        // `initRcvRatchet_` keeps the connection's setting even when the reply brings no KEM.
        let pq = PqSupport(rec.pq);
        let init = pq_x3dh_rcv(&x3dh, &snd)?;
        let mut ratchet = Ratchet::init_rcv(RatchetVersions::new(snd.version), x3dh.pk2, init, pq);
        let plain = ratchet.decrypt(&conf.enc_conn_info)?;
        let AgentMessage::ConnInfoReply { queues, conn_info } = AgentMessage::from_bytes(&plain)?
        else {
            return Err(AgentError::Prohibited("expected AgentConnInfoReply"));
        };
        accept_chain(&mut rec, v, sender, conf.agent_version, &plain)?;
        rec.peer_reply = queues.into_iter().next();
        rec.status = ConnStatus::Confirmed;
        let id = rec.id;
        self.save(store, rec, Some(ratchet), Batch::default())?;
        self.emit(AgentEvent::Conf {
            conn: id,
            conn_info,
        });
        Ok(())
    }

    /// The owner's answer to a request that was ratchet-encrypted: it continues that ratchet, so it brings no params.
    fn answer_confirmed(
        &mut self,
        store: &mut Store,
        mut rec: ConnRecord,
        ratchet: Option<Ratchet>,
        v: Version,
        sender: PublicKey,
        conf: Confirmation,
    ) -> Result<()> {
        let mut ratchet = ratchet.ok_or(AgentError::Prohibited("no ratchet"))?;
        if conf.e2e.is_some() {
            return Err(AgentError::Prohibited(
                "ratchet params in the owner's answer",
            ));
        }
        let plain = ratchet.decrypt(&conf.enc_conn_info)?;
        let AgentMessage::ConnInfoReply { queues, conn_info } = AgentMessage::from_bytes(&plain)?
        else {
            return Err(AgentError::Prohibited("expected AgentConnInfoReply"));
        };
        accept_chain(&mut rec, v, sender, conf.agent_version, &plain)?;
        rec.peer_reply = queues.into_iter().next();
        rec.status = ConnStatus::Confirmed;
        let id = rec.id;
        self.save(store, rec, Some(ratchet), Batch::default())?;
        self.emit(AgentEvent::Conf {
            conn: id,
            conn_info,
        });
        Ok(())
    }

    /// The joiner's side: the inviter's info completes the connection.
    fn inviter_confirmed(
        &mut self,
        store: &mut Store,
        mut rec: ConnRecord,
        mut ratchet: Ratchet,
        v: Version,
        sender: PublicKey,
        conf: Confirmation,
    ) -> Result<()> {
        if conf.e2e.is_some() {
            return Err(AgentError::Prohibited(
                "ratchet params in the inviter's confirmation",
            ));
        }
        let plain = ratchet.decrypt(&conf.enc_conn_info)?;
        let id = rec.id;
        let AgentMessage::ConnInfo(conn_info) = AgentMessage::from_bytes(&plain)? else {
            return Err(AgentError::Prohibited("expected AgentConnInfo"));
        };
        accept_chain(&mut rec, v, sender, conf.agent_version, &plain)?;
        rec.status = ConnStatus::Active;
        self.save(store, rec, Some(ratchet), Batch::default())?;
        self.emit(AgentEvent::Info {
            conn: id,
            conn_info,
        });
        self.emit(AgentEvent::Con { conn: id });
        Ok(())
    }
}

/// The confirmation is the peer's message 1; its key opens what follows on the queue.
fn accept_chain(
    rec: &mut ConnRecord,
    v: Version,
    sender: PublicKey,
    agent_v: Version,
    plain: &[u8],
) -> Result<()> {
    let rq = rec.rcv.as_mut().ok_or(AgentError::Prohibited("no queue"))?;
    check_conf_versions(rq.client_version, v, agent_v)?;
    rq.peer_e2e = Some(sender);
    rq.client_version = rq.client_version.min(v);
    rec.agent_version = rec.agent_version.min(agent_v);
    rec.chain.rcv_id = 1;
    rec.chain.rcv_hash = msg_hash(plain).to_vec();
    Ok(())
}

/// `checkConfVersions`: the header's client version, outside the box, may not fall below what
/// the agent version requires.
fn check_conf_versions(agreed: Version, v: Version, agent_v: Version) -> Result<()> {
    if agent_v < MIN_SUPPORTED_AGENT || v < agreed.min(min_client_version(agent_v)) {
        return Err(AgentError::Version);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_confirmation_header_below_the_agent_versions_client_minimum_is_refused() {
        let ok =
            |agreed, v, agent| check_conf_versions(Version(agreed), Version(v), Version(agent));
        assert_eq!(ok(4, 3, 8), Err(AgentError::Version));
        assert_eq!(ok(4, 2, 7), Err(AgentError::Version));
        assert_eq!(ok(4, 4, 8), Ok(()));
        assert_eq!(ok(4, 3, 7), Ok(()));
        assert_eq!(ok(2, 2, 8), Ok(()), "never above what was agreed");
        assert_eq!(ok(4, 4, 5), Err(AgentError::Version));
    }
}
