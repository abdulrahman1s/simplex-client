//! A signed (`/`) element checked against its author's member key (Subscriber.hs `withVerifiedMsg`, `verifyGroupSig`; Internal.hs `groupBindingData`).

use crate::types::model::ChatNotice;

use crate::chat::ids::{MemberId, b64_decode};
use crate::chat::signed::{ChatBinding, SignedMsg};
use crate::crypto::{PublicKey, Signature};
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::machine::Engine;
use crate::engine::records::ItemRecord;

use super::inbound::GroupCtx;

/// `groupBindingData` of a classic group: `smpEncode (memberId, memberKey)`, each a
/// ByteString behind its one-byte length, the key as its DER.
fn binding_data(member_id: &MemberId, der: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    for part in [member_id.as_bytes(), der] {
        out.push(u8::try_from(part.len()).ok()?);
        out.extend_from_slice(part);
    }
    Some(out)
}

/// `verifyMemberSig`: every signature verifies over `'G' ‖ binding data ‖ signed body`.
#[must_use]
pub(crate) fn signed_by(member_id: &MemberId, key_b64: &str, signed: &SignedMsg) -> bool {
    let Ok(der) = b64_decode(key_b64) else {
        return false;
    };
    let Ok(key @ PublicKey::Ed25519(_)) = PublicKey::from_der(&der) else {
        return false;
    };
    let Some(data) = binding_data(member_id, &der) else {
        return false;
    };
    let input = signed.signed_input(&data);
    signed.signatures.iter().all(|s| {
        <[u8; 64]>::try_from(s.signature.as_slice())
            .is_ok_and(|raw| key.verify(&Signature::Ed25519(raw), &input))
    })
}

/// `MsgSigStatus`: how a signed element stood once checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SigStatus {
    Verified,
    /// Signed, but there is no key here to check it with, or not for a group.
    SignedNoKey,
}

impl<A: AgentPort> Engine<A> {
    /// `withVerifiedMsg` in a classic group, where a signature is optional: an unsigned
    /// element, or one whose author has no key here, is taken; a group signature from a
    /// member whose key is known must verify. One that does not is rejected with a "bad
    /// signature" item from its author at `at`.
    pub(crate) fn check_signature(
        &mut self,
        group: i64,
        author: i64,
        signed: Option<&SignedMsg>,
        at: i64,
    ) -> Result<Option<SigStatus>> {
        let Some(sm) = signed else {
            return Ok(None);
        };
        let m = self.member(group, author)?;
        let (Some(key), ChatBinding::Group) = (&m.key, sm.binding) else {
            return Ok(Some(SigStatus::SignedNoKey));
        };
        if signed_by(&m.member_id, key, sm) {
            return Ok(Some(SigStatus::Verified));
        }
        self.bad_signature(group, Some(author), at)?;
        Err(fail(Code::RequestFailed, "message rejected: bad signature"))
    }

    /// `createInternalChatItem … RGEMsgBadSignature`.
    pub(crate) fn bad_signature(&mut self, group: i64, author: Option<i64>, at: i64) -> Result<()> {
        self.group_notice_at(group, author, ChatNotice::BadSignature, at)
    }

    /// `requireVerifiedEdit`, `requireVerifiedDelete`: a change to an item whose signature
    /// verified must verify too. An unsigned one is a forgery, shown as a bad signature from
    /// `shown_as`; a signed one with no key to check it is dropped.
    pub(crate) fn require_verified(
        &mut self,
        ctx: &GroupCtx,
        item: &ItemRecord,
        shown_as: Option<i64>,
        at: i64,
    ) -> Result<()> {
        if !item.verified {
            return Ok(());
        }
        match ctx.signed {
            Some(SigStatus::Verified) => Ok(()),
            Some(SigStatus::SignedNoKey) => Err(fail(
                Code::RequestFailed,
                "unverified change of a signed item (no key to verify), dropped",
            )),
            None => {
                self.bad_signature(ctx.group, shown_as, at)?;
                Err(fail(Code::RequestFailed, "message rejected: bad signature"))
            }
        }
    }
}
