//! Ed25519 on raw key bytes (Crypto.hs `Ed25519` instances); sizes in `alg.rs`.
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};

pub(super) fn public(seed: &[u8; 32]) -> [u8; 32] {
    SigningKey::from_bytes(seed).verifying_key().to_bytes()
}

pub(super) fn sign(seed: &[u8; 32], msg: &[u8]) -> [u8; 64] {
    SigningKey::from_bytes(seed).sign(msg).to_bytes()
}

/// Strict: a small-order key or `R` would let one signature verify every message.
pub(super) fn verify(public: &[u8; 32], sig: &[u8; 64], msg: &[u8]) -> bool {
    VerifyingKey::from_bytes(public)
        .is_ok_and(|vk| vk.verify_strict(msg, &Signature::from_bytes(sig)).is_ok())
}
