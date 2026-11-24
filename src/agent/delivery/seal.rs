//! The per-queue crypto_box around agent envelopes, both ways (Agent/Client.hs `agentCbEncrypt`, `agentCbDecrypt`; HANDSHAKE.md §2.8, §3.2).
use crate::crypto::{Alg, CbNonce, KeyPair, PublicKey, cb_decrypt, cb_encrypt, dh};
use crate::encoding::{Decode, Encode, Version};

use super::client_msg::{ClientMessage, ClientMsgEnvelope, PrivHeader};
use crate::agent::SmpQueueInfo;
use crate::agent::error::{AgentError, Result};
use crate::agent::limits::{E2E_ENC_CONFIRMATION_LENGTH, E2E_ENC_MESSAGE_LENGTH};
use crate::agent::records::{RcvQueue, SndQueue};

/// The SEND body for `envelope`. A confirmation carries our e2e key, from which the recipient derives the secret.
pub fn seal(sq: &SndQueue, envelope: &[u8], confirmation: bool) -> Result<Vec<u8>> {
    let nonce = CbNonce::random()?;
    let msg = ClientMessage {
        header: PrivHeader::Empty,
        body: envelope.to_vec(),
    };
    let len = if confirmation {
        E2E_ENC_CONFIRMATION_LENGTH
    } else {
        E2E_ENC_MESSAGE_LENGTH
    };
    let enc_body = cb_encrypt(&sq.e2e_secret()?, &nonce, &msg.to_bytes()?, len)?;
    let e2e_pub = confirmation.then(|| sq.e2e_priv.public()).transpose()?;
    Ok(ClientMsgEnvelope {
        version: sq.client_version,
        e2e_pub,
        nonce,
        enc_body,
    }
    .to_bytes()?)
}

/// The public header's key, read against the queue's state (Agent.hs `processClientMsg`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeaderKey {
    /// No key: the stored secret of a confirmed queue opened it.
    None,
    /// The sender's key on a queue not yet confirmed: a confirmation or a request.
    First(PublicKey),
    /// A key on a confirmed queue. The stored secret still opened it, so only a confirmation
    /// sent again after a lost ACK may carry one; a relay that knows the queue's `dh=` key
    /// must not get to pick the secret.
    Repeated,
}

/// What one queue message held.
pub struct Opened {
    pub version: Version,
    pub header: HeaderKey,
    pub envelope: Vec<u8>,
}

/// Opens a message on `rq`; sender-secured queues only take `PHEmpty`.
pub fn open(rq: &RcvQueue, body: &[u8]) -> Result<Opened> {
    let env = ClientMsgEnvelope::from_bytes(body)?;
    let (peer, header) = match (&rq.peer_e2e, env.e2e_pub) {
        (Some(k), None) => (k.clone(), HeaderKey::None),
        (Some(k), Some(_)) => (k.clone(), HeaderKey::Repeated),
        (None, Some(k)) => (k.clone(), HeaderKey::First(k)),
        (None, None) => return Err(AgentError::Prohibited("message before confirmation")),
    };
    let plain = cb_decrypt(&dh(&peer, &rq.e2e_priv)?, &env.nonce, &env.enc_body)?;
    let msg = ClientMessage::from_bytes(&plain)?;
    if msg.header != PrivHeader::Empty {
        return Err(AgentError::Prohibited(
            "legacy confirmation on a secured queue",
        ));
    }
    Ok(Opened {
        version: env.version,
        header,
        envelope: msg.body,
    })
}

/// `agentCbEncryptOnce`: a message to a queue we have no secret with (an address): a fresh key pair, the public half in the header.
pub fn seal_once(queue: &SmpQueueInfo, envelope: &[u8]) -> Result<Vec<u8>> {
    let ephemeral = KeyPair::generate(Alg::X25519)?;
    let secret = dh(&queue.address.dh_public_key, &ephemeral.private)?;
    let nonce = CbNonce::random()?;
    let msg = ClientMessage {
        header: PrivHeader::Empty,
        body: envelope.to_vec(),
    };
    let enc_body = cb_encrypt(
        &secret,
        &nonce,
        &msg.to_bytes()?,
        E2E_ENC_CONFIRMATION_LENGTH,
    )?;
    Ok(ClientMsgEnvelope {
        version: queue.client_version,
        e2e_pub: Some(ephemeral.public),
        nonce,
        enc_body,
    }
    .to_bytes()?)
}
