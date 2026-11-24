//! Which waiting call an engine event answers, and with what.
use serde_json::{Value, json};

use super::payload::Payload;
use super::waiters::Want;
use crate::types::{Event, RequestKind};

/// The want an event answers, `None` in it standing for any of that kind, and the answer.
pub(super) fn answer(e: &Event) -> Option<(Want, bool, Result<Payload, Value>)> {
    let ok = |want, exact, v| Some((want, exact, Ok(v)));
    let text = |t: &String| Payload::Json(json!(t));
    match e {
        Event::Invite(link) => ok(Want::Invite, true, text(link)),
        Event::Address(Some(link)) => ok(Want::Address, true, text(link)),
        Event::History { tag, messages, .. } => ok(
            Want::History(*tag),
            true,
            Payload::Messages(messages.clone()),
        ),
        Event::Members { group, members } => {
            let list = members.clone();
            let members = Payload::Members {
                group: *group,
                list,
            };
            ok(Want::Members(*group), true, members)
        }
        Event::Code { friend, code } => ok(Want::Code(*friend), true, text(code)),
        Event::Failed {
            request,
            code,
            detail,
        } => {
            let want = match request {
                RequestKind::CreateInvite => Want::Invite,
                RequestKind::Address => Want::Address,
                RequestKind::History => Want::History(0),
                RequestKind::Members => Want::Members(0),
                RequestKind::Code => Want::Code(0),
                // Only `createGroup` waits on a group request; the others are told.
                RequestKind::Group => Want::NewGroup,
                RequestKind::Start
                | RequestKind::SetProfile
                | RequestKind::Connect
                | RequestKind::Answer
                | RequestKind::Send
                | RequestKind::MarkRead
                | RequestKind::Refresh
                | RequestKind::RemoveFriend
                | RequestKind::SendFile
                | RequestKind::Edit
                | RequestKind::Delete
                | RequestKind::React
                | RequestKind::Mute
                | RequestKind::Block
                | RequestKind::File
                | RequestKind::GroupLink
                | RequestKind::Call => return None,
            };
            let why = json!({ "code": code.name(), "detail": detail });
            Some((want, false, Err(why)))
        }
        Event::Features { .. }
        | Event::Ready(_)
        | Event::Profile(_)
        | Event::Chats { .. }
        | Event::Address(None)
        | Event::Messages(_)
        | Event::Updated(_)
        | Event::Verified { .. }
        | Event::Offline(_)
        | Event::Preview { .. }
        | Event::OpenFile { .. }
        | Event::Located { .. }
        | Event::GroupLink { .. }
        | Event::Call(_)
        | Event::Stream(_)
        | Event::Exited { .. } => None,
    }
}
