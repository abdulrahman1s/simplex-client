//! The rules a request can be held to before the engine looks anything up: what the official client refuses on sight.

use crate::types::model::ChatRef;
use crate::types::{ErrorCode, Request};

use super::{MAX_ENCODED_MSG_LENGTH, MAX_SENT_MENTIONS, is_valid_name, json_len, wire_reaction};

/// Why a request breaks a limit: one of the limit codes, and words for a log.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Refusal {
    pub code: ErrorCode,
    pub detail: String,
}

fn refuse(code: ErrorCode, detail: impl Into<String>) -> Result<(), Refusal> {
    Err(Refusal {
        code,
        detail: detail.into(),
    })
}

/// A message's text alone longer than a whole message may be (`SELargeMsg`); the engine
/// checks the message as sent.
fn text_fits(text: &str) -> Result<(), Refusal> {
    if json_len(text) > MAX_ENCODED_MSG_LENGTH {
        return refuse(ErrorCode::TextTooLong, "message too large");
    }
    Ok(())
}

/// Checks what a request carries against the limits that need no state: the reaction set,
/// text size, the mention count in a group and a new group's name (trimmed, as the engine
/// keeps it). The engine checks the rest (roles, preferences, the reactions already given,
/// the edit window, file sizes, a changed display name or bio) when it handles the request.
pub fn check(request: &Request) -> Result<(), Refusal> {
    match request {
        Request::React { emoji, .. } => {
            if wire_reaction(emoji).is_none() {
                return refuse(
                    ErrorCode::ReactionNotAllowed,
                    format!("not one of the reactions: {emoji}"),
                );
            }
            Ok(())
        }
        Request::Send {
            chat,
            text,
            mentions,
            ..
        } => {
            if matches!(chat, ChatRef::Group(_)) && mentions.len() > MAX_SENT_MENTIONS {
                return refuse(ErrorCode::TooManyMentions, "more than 3 mentions");
            }
            text_fits(text)
        }
        Request::Edit { text, .. }
        | Request::SendFile { text, .. }
        | Request::SendLink { text, .. } => text_fits(text),
        Request::NewGroup(name) | Request::SetGroupProfile { name, .. } => {
            if !is_valid_name(name.trim()) {
                return refuse(
                    ErrorCode::InvalidName,
                    format!("not a valid name: {name:?}"),
                );
            }
            Ok(())
        }
        Request::SetProfile(_)
        | Request::SetPicture(_)
        | Request::SetGroupPicture { .. }
        | Request::CreateInvite
        | Request::Connect(_)
        | Request::Accept(_)
        | Request::Reject(_)
        | Request::CreateAddress
        | Request::DeleteAddress
        | Request::ReceiveFile { .. }
        | Request::CancelFile { .. }
        | Request::OpenFile { .. }
        | Request::LocateFile { .. }
        | Request::CreateGroupLink(_)
        | Request::DeleteGroupLink(_)
        | Request::Delete { .. }
        | Request::SetMuted { .. }
        | Request::SetTimer { .. }
        | Request::SetPrefs { .. }
        | Request::SetMemberRole { .. }
        | Request::BlockMemberForAll { .. }
        | Request::RemoveMember { .. }
        | Request::DeleteGroup(_)
        | Request::SetGroupPrefs { .. }
        | Request::Moderate { .. }
        | Request::DeleteForMe { .. }
        | Request::ClearChat(_)
        | Request::MarkReadUpTo { .. }
        | Request::SetUnreadMark { .. }
        | Request::SetAlias { .. }
        | Request::SetMessageTtl { .. }
        | Request::SetBlocked { .. }
        | Request::SetMemberBlocked { .. }
        | Request::ShowCode(_)
        | Request::Verify { .. }
        | Request::Members(_)
        | Request::MarkRead(_)
        | Request::History { .. }
        | Request::Refresh
        | Request::AddMember { .. }
        | Request::JoinGroup(_)
        | Request::LeaveGroup(_)
        | Request::RemoveFriend(_)
        | Request::Call(_)
        | Request::Stream(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests;
