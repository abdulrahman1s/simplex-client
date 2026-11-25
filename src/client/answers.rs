//! Which waiting call an engine event answers.

use super::waiters::Want;
use crate::types::{Event, RequestKind};

/// The slot an event answers, and the exact want it must match (`None`: whoever waits there).
pub(super) fn answers(event: &Event) -> Option<(usize, Option<Want>)> {
    let exact = |w: Want| Some((w.slot(), Some(w)));
    match event {
        Event::Invite(_) => exact(Want::Invite),
        Event::Address(Some(_)) => exact(Want::Address),
        Event::History { tag, .. } => exact(Want::History(*tag)),
        Event::Members { group, .. } => exact(Want::Members(*group)),
        Event::Code { friend, .. } => exact(Want::Code(*friend)),
        Event::Failed { request, .. } => failed_slot(*request).map(|s| (s, None)),
        Event::Features { .. }
        | Event::Ready(_)
        | Event::Profile(_)
        | Event::Chats { .. }
        | Event::Address(None)
        | Event::Messages(_)
        | Event::Updated(_)
        | Event::Verified { .. }
        | Event::Offline(_)
        | Event::Cleared(_)
        | Event::Preview { .. }
        | Event::OpenFile { .. }
        | Event::Located { .. }
        | Event::GroupLink { .. }
        | Event::Call(_)
        | Event::Stream(_)
        | Event::Exited { .. } => None,
    }
}

fn failed_slot(kind: RequestKind) -> Option<usize> {
    use RequestKind as K;
    let want = match kind {
        K::CreateInvite => Want::Invite,
        K::Address => Want::Address,
        K::History => Want::History(0),
        K::Members => Want::Members(0),
        K::Code => Want::Code(0),
        // Only `create_group` waits on a group request; the others are told, not asked.
        K::Group => Want::NewGroup,
        K::Start
        | K::SetProfile
        | K::Connect
        | K::Answer
        | K::Send
        | K::MarkRead
        | K::Refresh
        | K::RemoveFriend
        | K::SendFile
        | K::Edit
        | K::Delete
        | K::React
        | K::Mute
        | K::Timer
        | K::Prefs
        | K::GroupAdmin
        | K::Local
        | K::Block
        | K::File
        | K::GroupLink
        | K::Call => return None,
    };
    Some(want.slot())
}
