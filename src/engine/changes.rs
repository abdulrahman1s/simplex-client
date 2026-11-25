//! What changed in the chat list since it last went out, so a client hearing deltas gets only that (`Event::ChatsChanged`).

use std::collections::BTreeSet;
use std::mem;

use crate::types::Event;

use super::agent_port::AgentPort;
use super::machine::Engine;

/// Marked where a record the list shows is saved or dropped (`load.rs`), so nothing that
/// changes the list goes unmarked. Eviction: emptied by each list or change that goes out;
/// bounded by the friends and groups.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    /// The whole list is due: a refresh, or the user's preferences, which every friend shows.
    pub all: bool,
    pub friends: BTreeSet<i64>,
    /// A group, or one of its members: the list counts them.
    pub groups: BTreeSet<i64>,
    pub requests: bool,
}

impl Changes {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        !self.all && self.friends.is_empty() && self.groups.is_empty() && !self.requests
    }
}

impl<A: AgentPort> Engine<A> {
    /// Whether the list's next event is due: a whole list when it changed, or with deltas
    /// whatever was marked (previews alone wait for a whole list).
    pub(super) fn list_due(&self) -> bool {
        if self.deltas {
            !self.state.changed.is_empty()
        } else {
            self.state.chats_dirty
        }
    }

    /// The list's next event, built only when it is next so changes made while it waits go
    /// out together.
    pub(super) fn list_out(&mut self) -> Option<Event> {
        if !self.list_due() {
            return None;
        }
        self.state.chats_dirty = false;
        let changed = mem::take(&mut self.state.changed);
        if !self.deltas || changed.all {
            return Some(self.chats());
        }
        Some(self.chats_changed(&changed))
    }

    fn chats_changed(&self, c: &Changes) -> Event {
        let (mut friends, mut removed_friends) = (Vec::new(), Vec::new());
        for id in &c.friends {
            match self.state.friends.get(id) {
                Some(f) => friends.push(self.friend_wire(f)),
                None => removed_friends.push(*id),
            }
        }
        let (mut groups, mut removed_groups) = (Vec::new(), Vec::new());
        for id in &c.groups {
            match self.state.groups.get(id) {
                Some(g) => groups.push(self.group_wire(g)),
                None => removed_groups.push(*id),
            }
        }
        Event::ChatsChanged {
            friends,
            groups,
            removed_friends,
            removed_groups,
            requests: c.requests.then(|| self.requests_wire()),
        }
    }
}
