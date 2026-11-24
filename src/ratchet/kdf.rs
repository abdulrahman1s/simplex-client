//! Root and chain KDFs of the double ratchet with header encryption (Ratchet.hs `rootKdf`, `chainKdf`, `hkdf3`).
use zeroize::Zeroizing;

use super::error::Result;
use super::kem::KemSharedSecret;
use super::key::Key32;
use crate::crypto::{PrivateKey, PublicKey, dh, hkdf_sha512};

/// A one-time AES key and its 16-byte IV (Ratchet.hs `MessageKey`).
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MessageKey {
    pub key: Key32,
    pub iv: [u8; 16],
}

/// Three 32-byte outputs of one HKDF-SHA512 call, kept in the wiped buffer until taken as keys.
pub(crate) struct Hkdf3(Zeroizing<[u8; 96]>);

impl Hkdf3 {
    pub(crate) fn key(&self, i: usize) -> Key32 {
        let mut k = Key32::new([0; 32]);
        k.fill(&self.0[i * 32..(i + 1) * 32]);
        k
    }

    fn bytes(&self, i: usize) -> &[u8] {
        &self.0[i * 32..(i + 1) * 32]
    }
}

pub(crate) fn hkdf3(salt: &[u8], ikm: &[u8], info: &[u8]) -> Result<Hkdf3> {
    Ok(Hkdf3(hkdf_sha512::<96>(salt, ikm, info)?))
}

/// `rootKdf`: the new root key, the chain key and the next header key.
pub(crate) fn root_kdf(
    rk: &Key32,
    peer: &PublicKey,
    own: &PrivateKey,
    kem: Option<&KemSharedSecret>,
) -> Result<(Key32, Key32, Key32)> {
    let mut ikm = Zeroizing::new(dh(peer, own)?.as_bytes().to_vec());
    if let Some(ss) = kem {
        ikm.extend_from_slice(ss.as_ref());
    }
    let out = hkdf3(rk.as_bytes(), &ikm, b"SimpleXRootRatchet")?;
    Ok((out.key(0), out.key(1), out.key(2)))
}

pub(crate) struct ChainStep {
    pub next: Key32,
    pub message: MessageKey,
    pub header_iv: [u8; 16],
}

/// `chainKdf`: the next chain key, the message key with its IV, and the header IV.
pub(crate) fn chain_kdf(ck: &Key32) -> Result<ChainStep> {
    let out = hkdf3(&[], ck.as_bytes(), b"SimpleXChainRatchet")?;
    let (mut iv, mut header_iv) = ([0u8; 16], [0u8; 16]);
    iv.copy_from_slice(&out.bytes(2)[..16]);
    header_iv.copy_from_slice(&out.bytes(2)[16..]);
    Ok(ChainStep {
        next: out.key(0),
        message: MessageKey {
            key: out.key(1),
            iv,
        },
        header_iv,
    })
}
