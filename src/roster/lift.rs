//! The engine's events as the high-level clients report them: one message per event, and the chat list as changes.

use super::Roster;
use crate::types::Event;
use crate::types::model::{
    ChatGroup, ChatMessage, ChatNotice, ChatProfile, ChatRef, Friend, FriendRequest, MessageBody,
};

/// One thing a high-level client reports, carrying only data: the clients attach themselves.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Note {
    Ready(ChatProfile),
    Message(ChatMessage),
    MessageUpdate(ChatMessage),
    Request(FriendRequest),
    FriendAdd(Friend),
    FriendRemove(Friend),
    GroupAdd(ChatGroup),
    GroupRemove(ChatGroup),
    GroupInvite(ChatGroup),
    /// A member connected with the user (`MemberJoined`): someone new to the group, or each
    /// member already there when the user joins. The engine names them only.
    MemberAdd {
        group: i64,
        name: String,
    },
    Offline(bool),
    Data {
        chat: ChatRef,
        member: Option<i64>,
        tag: String,
        json: serde_json::Map<String, serde_json::Value>,
    },
}

/// What `e` means to a high-level client; the chat list also updates `roster`.
pub(crate) fn lift(e: &Event, roster: &mut Roster) -> Vec<Note> {
    match e {
        Event::Ready(p) => vec![Note::Ready(p.clone())],
        Event::Messages(list) => list.iter().flat_map(arrived).collect(),
        Event::Updated(list) => list.iter().cloned().map(Note::MessageUpdate).collect(),
        Event::Chats {
            friends,
            groups,
            requests,
            ..
        } => roster.update(friends, groups, requests),
        Event::Offline(offline) => vec![Note::Offline(*offline)],
        Event::Data {
            chat,
            member,
            tag,
            json,
        } => vec![Note::Data {
            chat: *chat,
            member: *member,
            tag: tag.clone(),
            json: json.clone(),
        }],
        Event::Cleared(_)
        | Event::Features { .. }
        | Event::Profile(_)
        | Event::Invite(_)
        | Event::Address(_)
        | Event::History { .. }
        | Event::Code { .. }
        | Event::Verified { .. }
        | Event::Members { .. }
        | Event::Preview { .. }
        | Event::OpenFile { .. }
        | Event::Located { .. }
        | Event::GroupLink { .. }
        | Event::Call(_)
        | Event::Stream(_)
        | Event::Failed { .. }
        | Event::Exited { .. } => Vec::new(),
    }
}

/// The message, and a member's arrival when it is the notice of one.
fn arrived(m: &ChatMessage) -> Vec<Note> {
    let mut out = vec![Note::Message(m.clone())];
    if let (ChatRef::Group(group), MessageBody::Notice(ChatNotice::MemberJoined(name))) =
        (m.chat, &m.body)
    {
        let name = name.clone();
        out.push(Note::MemberAdd { group, name });
    }
    out
}
