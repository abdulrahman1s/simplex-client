//! A channel's signing: the binding, signing a body and checking one (Internal.hs `groupBindingData`, `groupMsgSigning`; Protocol.hs `signChatMsgBody`; Subscriber.hs `verifyMemberSig`).

use crate::chat::ids::{MemberId, b64_decode, b64_encode};
use crate::chat::signed::{ChatBinding, MsgSignature, SignedMsg};
use crate::crypto::{Alg, KeyPair, PrivateKey, PublicKey, Signature};
use crate::engine::error::{Code, Result, fail};

fn key_err(what: &str) -> crate::engine::error::EngineError {
    fail(Code::RequestFailed, format!("channel key: {what}"))
}

/// `groupBindingData` of a channel: `smpEncode (publicGroupId, memberId)`, each behind its
/// one-byte length.
pub(crate) fn binding(public_group_id: &[u8], member_id: &MemberId) -> Result<Vec<u8>> {
    let mut out = Vec::new();
    for part in [public_group_id, member_id.as_bytes()] {
        out.push(u8::try_from(part.len()).map_err(|_| key_err("binding part too long"))?);
        out.extend_from_slice(part);
    }
    Ok(out)
}

/// A new Ed25519 pair: the private half as PKCS#8 DER, the public half as the wire's base64url DER.
pub(crate) fn new_pair() -> Result<(Vec<u8>, String)> {
    let pair = KeyPair::generate(Alg::Ed25519).map_err(|_| key_err("no randomness"))?;
    Ok((
        pair.private.to_der().to_vec(),
        b64_encode(&pair.public.to_der()),
    ))
}

pub(crate) fn private(der: &[u8]) -> Result<PrivateKey> {
    PrivateKey::from_der(der).map_err(|_| key_err("bad private key"))
}

/// The wire form (`MemberKey`) of a private key's public half.
pub(crate) fn public_b64(der: &[u8]) -> Result<String> {
    let public = private(der)?
        .public()
        .map_err(|_| key_err("bad private key"))?;
    Ok(b64_encode(&public.to_der()))
}

/// `signChatMsgBody` with `CBGroup`: one `KRMember` signature over `'G' ‖ binding ‖ body`.
pub(crate) fn sign(key_der: &[u8], binding: &[u8], body: Vec<u8>) -> Result<SignedMsg> {
    let mut signed = SignedMsg {
        binding: ChatBinding::Group,
        signatures: Vec::new(),
        signed_body: body,
    };
    let sig = private(key_der)?
        .sign(&signed.signed_input(binding))
        .map_err(|_| key_err("signing failed"))?;
    signed.signatures.push(MsgSignature {
        signature: sig.as_bytes().to_vec(),
    });
    Ok(signed)
}

/// `verifyMemberSig`: every signature verifies under the member's key (wire base64url DER).
#[must_use]
pub(crate) fn verified(key_b64: &str, binding: &[u8], signed: &SignedMsg) -> bool {
    let Ok(key @ PublicKey::Ed25519(_)) = b64_decode(key_b64)
        .ok()
        .and_then(|d| PublicKey::from_der(&d).ok())
        .ok_or(())
    else {
        return false;
    };
    let input = signed.signed_input(binding);
    !signed.signatures.is_empty()
        && signed.signatures.iter().all(|s| {
            Signature::from_raw(Alg::Ed25519, &s.signature)
                .is_ok_and(|sig| key.verify(&sig, &input))
        })
}
