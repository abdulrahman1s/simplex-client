//! The chat list: every friend and group, and a conversation for each friend not blocked and each group (codec `parse_chats`).

use crate::types::Event;
use crate::types::model::{ChatPreview, ChatRef, Friend, FriendRequest};

use super::agent_port::AgentPort;
use super::machine::Engine;
use super::prefs::wire::to_wire;
use super::profile::picture_of;
use super::records::FriendRecord;
use super::timed::rules::friend_ttl;
use crate::chat::preferences::Preferences;

fn friend(user: Option<&Preferences>, f: &FriendRecord) -> Friend {
    Friend {
        id: f.id,
        name: f.name.clone(),
        ready: f.reachable(),
        about: f.about.clone(),
        since: Some(f.since),
        blocked: f.blocked,
        muted: f.muted,
        verified: f.verified.is_some(),
        picture: f.image.as_deref().and_then(picture_of),
        timer: friend_ttl(user, f).flatten(),
        alias: f.local.alias.clone(),
        item_ttl: f.local.item_ttl,
        address: f.address.clone(),
        incognito: f.incognito.clone(),
        pq: f.pq_on,
        prefs: to_wire(f.my_prefs.as_ref()),
        ..Friend::default()
    }
}

impl<A: AgentPort> Engine<A> {
    /// A friend as the list shows them.
    pub(super) fn friend_wire(&self, f: &FriendRecord) -> Friend {
        Friend {
            features: self.features(f),
            ..friend(self.state.user.prefs.as_ref(), f)
        }
    }

    pub(super) fn requests_wire(&self) -> Vec<FriendRequest> {
        (self.state.address.requests.values())
            .map(|r| FriendRequest {
                id: r.id,
                name: r.name.clone(),
            })
            .collect()
    }

    pub(crate) fn chats(&self) -> Event {
        let friends = (self.state.friends.values())
            .map(|f| self.friend_wire(f))
            .collect();
        // A blocked friend is reachable from "Blocked", not from the list.
        let direct = self
            .state
            .friends
            .values()
            .filter(|f| !f.blocked)
            .map(|f| ChatPreview {
                chat: ChatRef::Direct(f.id),
                name: f.name.clone(),
                last: self.state.last.get(&ChatRef::Direct(f.id)).cloned(),
                unread: f.unread,
                muted: f.muted,
                marked_unread: f.local.marked_unread,
            });
        let in_groups = self.state.groups.values().map(|g| ChatPreview {
            chat: ChatRef::Group(g.id),
            name: g.profile.display_name.clone(),
            last: self.state.last.get(&ChatRef::Group(g.id)).cloned(),
            unread: g.unread,
            muted: g.muted,
            marked_unread: g.local.marked_unread,
        });
        let chats = direct.chain(in_groups).collect();
        let groups = self
            .state
            .groups
            .values()
            .map(|g| self.group_wire(g))
            .collect();
        let requests = self.requests_wire();
        Event::Chats {
            friends,
            groups,
            requests,
            chats,
        }
    }
}
