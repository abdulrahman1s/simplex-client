//! What the engine thread keeps in memory: a cache of the records, rebuilt from the store at start.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::types::model::{ChatMessage, ChatRef};

use super::address::AddressState;
use super::agent_port::ConnId;
use super::calls::CallState;
use super::records::{
    ConnKind, ConnRecord, FriendRecord, GroupRecord, MemberRecord, SettingsRecord, UserRecord,
};

#[derive(Debug, Default)]
pub struct State {
    pub user: UserRecord,
    pub settings: SettingsRecord,
    /// Eviction: a friend leaves with `RemoveFriend`.
    pub friends: BTreeMap<i64, FriendRecord>,
    /// Eviction: with its friend, or when the invitation is evicted (`MAX_INVITATIONS`).
    pub conns: HashMap<ConnId, ConnRecord>,
    /// Eviction: a group leaves when the user deletes it.
    pub groups: BTreeMap<i64, GroupRecord>,
    /// Every group's members other than the user, by (group, member). Eviction: with the group.
    pub members: BTreeMap<(i64, i64), MemberRecord>,
    /// Each conversation's newest item, for the chat list. Eviction: with the friend or group.
    pub last: HashMap<ChatRef, ChatMessage>,
    /// Relay hosts the agent reported and whether each is up. Bounded by the relays in use.
    pub hosts: HashMap<String, bool>,
    pub offline: bool,
    /// The invitation being made: one at a time.
    pub inviting: Option<ConnId>,
    /// The chat list changed and is due out; at most one is pending.
    pub chats_dirty: bool,
    pub address: AddressState,
    pub calls: CallState,
    /// Timed items with a delete time, earliest first: (delete at, chat, item), as the store's
    /// `timed_due` keys hold them. Eviction: an entry leaves when its item goes, by its timer or
    /// any other deletion, or with its chat.
    pub timed: BTreeSet<(i64, ChatRef, i64)>,
    /// Received timed items not read yet, by chat, as the store's `timed_unread` keys hold
    /// them. Eviction: an entry leaves when its item is read or goes, or with its chat.
    pub timed_unread: BTreeSet<(ChatRef, i64)>,
}

impl State {
    #[must_use]
    pub fn friend_of(&self, conn: &ConnId) -> Option<i64> {
        match self.conns.get(conn)?.kind {
            ConnKind::Friend { id } => Some(id),
            ConnKind::Invitation { .. } | ConnKind::Member { .. } | ConnKind::GroupLink { .. } => {
                None
            }
        }
    }

    /// The `pqEncryption` a message on `conn` goes with: a friend's own; group connections are PQ off.
    #[must_use]
    pub fn pq_of(&self, conn: &ConnId) -> bool {
        let friend = self.friend_of(conn).and_then(|id| self.friends.get(&id));
        friend.is_some_and(|f| f.pq)
    }

    /// The (group, member) a group connection is with.
    #[must_use]
    pub fn member_of(&self, conn: &ConnId) -> Option<(i64, i64)> {
        match self.conns.get(conn)?.kind {
            ConnKind::Member { group, member } => Some((group, member)),
            ConnKind::Invitation { .. } | ConnKind::Friend { .. } | ConnKind::GroupLink { .. } => {
                None
            }
        }
    }

    /// A group's members other than the user.
    pub fn members_of(&self, group: i64) -> impl Iterator<Item = &MemberRecord> {
        self.members
            .range((group, i64::MIN)..=(group, i64::MAX))
            .map(|(_, m)| m)
    }
}
