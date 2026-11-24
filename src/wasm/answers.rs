//! Which waiting call an engine event answers, and with what.
use serde_json::{Value, json};

use super::sugar::message_value;
use super::waiters::Want;
use crate::types::{Event, RequestKind};

/// The want an event answers, `None` in it standing for any of that kind, and the answer.
pub(super) fn answer(e: &Event) -> Option<(Want, bool, Result<Value, Value>)> {
    let ok = |want, exact, v| Some((want, exact, Ok(v)));
    match e {
        Event::Invite(link) => ok(Want::Invite, true, json!(link)),
        Event::Address(Some(link)) => ok(Want::Address, true, json!(link)),
        Event::History { tag, messages, .. } => {
            let list = messages.iter().map(message_value).collect();
            ok(Want::History(*tag), true, Value::Array(list))
        }
        Event::Members { group, members } => ok(Want::Members(*group), true, json!(members)),
        Event::Code { friend, code } => ok(Want::Code(*friend), true, json!(code)),
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
                RequestKind::Start
                | RequestKind::SetProfile
                | RequestKind::Connect
                | RequestKind::Answer
                | RequestKind::Send
                | RequestKind::MarkRead
                | RequestKind::Refresh
                | RequestKind::Group
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
            let code = serde_json::to_value(code).unwrap_or(Value::Null);
            let why = json!({ "code": code, "detail": detail });
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
