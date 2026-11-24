//! The destination relay's side of a forwarded command, for the fake servers (Server.hs `processForwardedCommand`).
use crate::crypto::{CbNonce, DhSecret, PrivateKey, cb_decrypt, cb_encrypt, dh};
use crate::smp::{
    BLOCK_SIZE, ErrorType, Incoming, Response, encode_transmission, pack, parse_payload,
};

use super::fwd::FwdTransmission;
use super::nonce::{reverse_nonce, transmission_nonce};
use crate::smp::version::{FWD_NONCES, PADDED_PROXIED_LENGTH};

/// One forwarded command, opened, and what sealing its answer needs.
pub struct Forwarded {
    pub t: Incoming,
    pub version: u16,
    secret: DhSecret,
    corr: CbNonce,
}

/// Opens `fwd` with the relay's key; the reference's errors are CRYPTO for what does not open
/// or names another corrId, BLOCK for anything but one transmission.
pub fn open_forwarded(
    server_key: &PrivateKey,
    fwd: &FwdTransmission,
) -> Result<Forwarded, ErrorType> {
    let corr = CbNonce::from_slice(&fwd.corr_id).map_err(|_| ErrorType::Crypto)?;
    let secret = dh(&fwd.key, server_key).map_err(|_| ErrorType::Crypto)?;
    let nonce = transmission_nonce(fwd.version, &corr);
    let plain = cb_decrypt(&secret, &nonce, &fwd.body).map_err(|_| ErrorType::Crypto)?;
    let mut ts = parse_payload(fwd.version, &plain).map_err(|_| ErrorType::Block)?;
    let (Some(t), None) = (ts.pop(), ts.pop()) else {
        return Err(ErrorType::Block);
    };
    if t.corr_id != fwd.corr_id {
        return Err(ErrorType::Crypto);
    }
    Ok(Forwarded {
        t,
        version: fwd.version,
        secret,
        corr,
    })
}

impl Forwarded {
    /// The PRES nonce and body answering with `r`.
    pub fn seal_answer(&self, r: &Response) -> (Option<CbNonce>, Vec<u8>) {
        let v = self.version;
        let cmd = r.encode(v).unwrap();
        let t = encode_transmission(v, &[], &self.t.corr_id, &self.t.entity_id, &cmd).unwrap();
        let (blocks, _) = pack(&[t], BLOCK_SIZE - 18);
        let nonce = (v >= FWD_NONCES).then(|| CbNonce::random().unwrap());
        let used = nonce.unwrap_or_else(|| reverse_nonce(&self.corr));
        let body = cb_encrypt(&self.secret, &used, &blocks[0], PADDED_PROXIED_LENGTH).unwrap();
        (nonce, body)
    }
}
