//! Whether a group message goes to a member now, waits for its connection, or is left to the member's inviter to forward (Internal.hs `memberSendAction`, classic groups).

use super::relay::forwardable;
use crate::chat::ChatEvent;
use crate::engine::agent_port::ConnId;
use crate::engine::records::{GroupRecord, MemberCategory, MemberRecord};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SendAction {
    Send(ConnId),
    /// Kept until the member's connection is up (`MSAPending`).
    Pending,
    /// Its inviter, also a recipient, passes it on (`MSAForwarded`).
    Forwarded,
}

/// `recipients`: every member the message goes to. `None`: the member gets nothing.
#[must_use]
pub(crate) fn send_action(
    g: &GroupRecord,
    m: &MemberRecord,
    recipients: &[i64],
    event: &ChatEvent,
) -> Option<SendAction> {
    // `connDisabled`, `ConnDeleted`, rejected: a member gone has no connection to wait for.
    if m.status.gone() {
        return None;
    }
    if let Some(conn) = m.reachable() {
        return Some(SendAction::Send(conn));
    }
    // `pendingOrForwarded`: who introduced the user to a pre-member is the user's host; a
    // post-member's is its announcer.
    let inviter = match m.category {
        MemberCategory::Invitee | MemberCategory::Host => return Some(SendAction::Pending),
        MemberCategory::Pre => g.host,
        MemberCategory::Post => m.invited_by,
    };
    if inviter.is_some_and(|i| recipients.contains(&i)) && forwardable(event) {
        return Some(SendAction::Forwarded);
    }
    if matches!(event, ChatEvent::GrpMsgForward { .. }) {
        return None;
    }
    Some(SendAction::Pending)
}
