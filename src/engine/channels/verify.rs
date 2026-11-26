//! A channel's signature rules: a signature from a known key must verify; the events that alter the channel must be signed (Subscriber.hs `withVerifiedMsg`, `verifyGroupSig`; Protocol.hs `requiresSignature`).

use crate::chat::ChatEvent;
use crate::chat::signed::{ChatBinding, SignedMsg};
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::groups::verify::SigStatus;
use crate::engine::machine::Engine;

use super::keys::{binding, verified};

/// `requiresSignature`: what must be signed in a group that uses relays.
#[must_use]
pub(crate) fn requires_signature(e: &ChatEvent) -> bool {
    matches!(
        e,
        ChatEvent::GrpDel
            | ChatEvent::GrpInfo(_)
            | ChatEvent::GrpPrefs(_)
            | ChatEvent::GrpMemDel { .. }
            | ChatEvent::GrpMemRole { .. }
            | ChatEvent::GrpMemRestrict { .. }
            | ChatEvent::GrpLeave
            | ChatEvent::GrpRelayNew { .. }
            | ChatEvent::GrpRoster { .. }
            | ChatEvent::Info { .. }
            | ChatEvent::GrpLinkMem { .. }
            | ChatEvent::GrpRelayInv(_)
    )
}

impl<A: AgentPort> Engine<A> {
    /// `withVerifiedMsg` in a channel. One rejected is shown as a bad signature from `author` at `at`.
    pub(crate) fn check_channel_signature(
        &mut self,
        group: i64,
        author: i64,
        signed: Option<&SignedMsg>,
        event: &ChatEvent,
        at: i64,
    ) -> Result<Option<SigStatus>> {
        let optional = !requires_signature(event);
        let g = self.group(group)?;
        let c = g
            .channel
            .as_ref()
            .ok_or_else(|| fail(Code::RequestFailed, "not a channel"))?;
        let m = self.member(group, author)?;
        let status = match (signed, &m.key) {
            (None, _) => optional.then_some(None),
            (Some(sm), Some(key)) if sm.binding == ChatBinding::Group => {
                let b = binding(&c.public_group_id, &m.member_id)?;
                verified(key, &b, sm).then_some(Some(SigStatus::Verified))
            }
            (Some(_), Some(_)) => optional.then_some(Some(SigStatus::SignedNoKey)),
            // `unverifiedAllowed`: a leave or profile between subscribers whose keys are not known.
            (Some(_), None) => (optional
                || matches!(event, ChatEvent::GrpLeave | ChatEvent::Info { .. }))
            .then_some(Some(SigStatus::SignedNoKey)),
        };
        match status {
            Some(s) => Ok(s),
            None => {
                self.bad_signature(group, Some(author), at)?;
                Err(fail(
                    Code::RequestFailed,
                    "message rejected: bad or missing signature",
                ))
            }
        }
    }
}
