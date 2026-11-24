//! A sender command sealed for the destination relay and wrapped in PFWD (Client.hs `proxySMPCommand`, its sending half).
use std::time::Duration;

use crate::crypto::{Alg, CbNonce, DhSecret, KeyPair, can_pad, cb_encrypt, dh};

use super::nonce::transmission_nonce;
use super::relay::ProxiedRelay;
use crate::smp::auth::Authorizer;
use crate::smp::commands::Command;
use crate::smp::error::SmpError;
use crate::smp::flow::Request;
use crate::smp::transmission::{encode_transmission, pack};
use crate::smp::version::{BLOCK_SIZE, PADDED_PROXIED_LENGTH};

/// What opening the PRES needs: the secret of this one command and its nonce.
pub struct Opener {
    pub(super) secret: DhSecret,
    pub(super) nonce: CbNonce,
    /// The relay's version, which the sealed answer is encoded in.
    pub(super) version: u16,
}

/// `batchTransmissions`' room: the block less its padding length, count byte and box tag.
const ROOM: usize = BLOCK_SIZE - 18;

/// Seals `request` (as it would go to the relay directly) for `relay`, with a fresh key so that
/// neither the proxy nor the relay can link it to the sender's other commands. The PFWD for the
/// proxy goes out under `timeout`, as the proxy waits for the relay in turn.
pub fn seal(
    relay: &ProxiedRelay,
    request: &Request,
    timeout: Duration,
) -> Result<(Request, Opener), SmpError> {
    let key = KeyPair::generate(Alg::X25519)?;
    let secret = dh(&relay.server_key, &key.private)?;
    let nonce = CbNonce::random()?;
    let corr = nonce.as_bytes().to_vec();
    let v = relay.version;
    let command = request.command.encode(v)?;
    let auth = Authorizer {
        session_id: &relay.session_id,
        server_key: &relay.server_key,
    };
    let auth = auth.authenticate(
        request.auth_key.as_ref(),
        &corr,
        &request.entity_id,
        &command,
    )?;
    let t = encode_transmission(v, &auth, &corr, &request.entity_id, &command)?;
    let (blocks, too_large) = pack(&[t], ROOM);
    let [block] = blocks.as_slice() else {
        return Err(SmpError::LargeMsg);
    };
    if !too_large.is_empty() || !can_pad(block.len(), PADDED_PROXIED_LENGTH) {
        return Err(SmpError::LargeMsg);
    }
    let body = cb_encrypt(
        &secret,
        &transmission_nonce(v, &nonce),
        block,
        PADDED_PROXIED_LENGTH,
    )?;
    let command = Command::Pfwd {
        version: v,
        key: key.public,
        body,
    };
    let mut pfwd = Request::new(command, relay.session_id.clone(), None).with_timeout(timeout);
    pfwd.corr_id = Some(corr);
    let opener = Opener {
        secret,
        nonce,
        version: v,
    };
    Ok((pfwd, opener))
}
