//! What `events()` yields: what happened, as objects to act on, and the engine's own events for the rest.

use super::handle::Client;
use super::objects::{Friend, FriendRequest, Group, GroupInvite, Member, Message};
use crate::roster::Note;
use crate::types;
use crate::types::model::ChatProfile;

/// One thing that happened. The objects it carries act through the client they came from.
#[derive(Clone, Debug)]
pub enum Event {
    /// The engine started with this profile.
    Ready(ChatProfile),
    /// A new message, the user's own included.
    Message(Message),
    /// A message changed: edited, deleted, reacted to, delivered, or a file's progress.
    MessageUpdate(Message),
    /// Someone asked to connect through the user's address.
    Request(FriendRequest),
    /// A friend finished connecting and can be written to.
    FriendAdd(Friend),
    /// A friend was removed, or removed the user.
    FriendRemove(Friend),
    /// The user made or joined a group.
    GroupAdd(Group),
    /// The user left or was removed from a group, or it was deleted.
    GroupRemove(Group),
    /// The user was invited into a group.
    GroupInvite(GroupInvite),
    /// A member connected with the user: someone new to the group, or, right after the user
    /// joins, each member already there. The engine names them only.
    MemberAdd(Member),
    /// No relay is reachable (`true`), or one is again (`false`).
    Offline(bool),
    /// An engine event without a variant of its own, and every chat list (`Chats`).
    Raw(types::Event),
}

/// An event as the dispatcher queues it: data only, so the queue never keeps the client alive.
pub(super) enum Queued {
    Note(Box<Note>),
    Raw(types::Event),
}

impl Event {
    pub(super) fn new(client: &Client, q: Queued) -> Self {
        match q {
            Queued::Note(note) => Self::from_note(client, *note),
            Queued::Raw(e) => Self::Raw(e),
        }
    }

    fn from_note(client: &Client, note: Note) -> Self {
        let c = || client.clone();
        match note {
            Note::Ready(p) => Self::Ready(p),
            Note::Message(m) => Self::Message(Message::new(c(), m)),
            Note::MessageUpdate(m) => Self::MessageUpdate(Message::new(c(), m)),
            Note::Request(r) => Self::Request(FriendRequest::new(c(), r)),
            Note::FriendAdd(f) => Self::FriendAdd(Friend::new(c(), f)),
            Note::FriendRemove(f) => Self::FriendRemove(Friend::new(c(), f)),
            Note::GroupAdd(g) => Self::GroupAdd(Group::new(c(), g)),
            Note::GroupRemove(g) => Self::GroupRemove(Group::new(c(), g)),
            Note::GroupInvite(g) => Self::GroupInvite(GroupInvite::new(Group::new(c(), g))),
            Note::MemberAdd { group, name } => Self::MemberAdd(Member::named(c(), group, name)),
            Note::Offline(offline) => Self::Offline(offline),
        }
    }
}
