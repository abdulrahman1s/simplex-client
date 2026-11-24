//! The nonces of a forwarded command and its answer (Protocol.hs `encTransmissionNonce`, Crypto.hs `xorNonce`, `reverseNonce`).
use crate::crypto::CbNonce;
use crate::smp::version::FWD_NONCES;

/// From v23 the relay's version is XORed into the first bytes, so a command sealed for one version does not open as another.
#[must_use]
pub fn transmission_nonce(version: u16, corr: &CbNonce) -> CbNonce {
    let mut n = *corr.as_bytes();
    if version >= FWD_NONCES {
        for (b, x) in n.iter_mut().zip(version.to_be_bytes()) {
            *b ^= x;
        }
    }
    CbNonce::new(n)
}

/// The nonce of an answer that names none.
#[must_use]
pub fn reverse_nonce(n: &CbNonce) -> CbNonce {
    let mut b = *n.as_bytes();
    b.reverse();
    CbNonce::new(b)
}
