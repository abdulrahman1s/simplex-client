//! The friends and groups the engine last listed, and making a group.

use std::sync::{MutexGuard, PoisonError};

use super::error::{Result, unexpected};
use super::handle::Client;
use super::objects::{Friend, Group};
use super::waiters::{Answer, Want};
use crate::roster::Roster;
use crate::types::model::{self as wire};
use crate::types::{Request, RequestKind};

impl Client {
    /// A friend from the last chat list, connected or still connecting.
    #[must_use]
    pub fn friend(&self, id: i64) -> Option<Friend> {
        let f = self.roster().friends.get(&id).cloned();
        f.map(|f| Friend::new(self.clone(), f))
    }

    #[must_use]
    pub fn friends(&self) -> Vec<Friend> {
        let list: Vec<_> = self.roster().friends.values().cloned().collect();
        list.into_iter()
            .map(|f| Friend::new(self.clone(), f))
            .collect()
    }

    /// A group from the last chat list, left ones included.
    #[must_use]
    pub fn group(&self, id: i64) -> Option<Group> {
        let g = self.roster().groups.get(&id).cloned();
        g.map(|g| Group::new(self.clone(), g))
    }

    #[must_use]
    pub fn groups(&self) -> Vec<Group> {
        let list: Vec<_> = self.roster().groups.values().cloned().collect();
        list.into_iter()
            .map(|g| Group::new(self.clone(), g))
            .collect()
    }

    /// Asks the engine for the whole chat list; it arrives as `Raw(Chats)`, with
    /// [`ClientBuilder::roster_deltas`](super::ClientBuilder::roster_deltas) too.
    pub fn refresh(&self) -> Result<()> {
        self.request(Request::Refresh)
    }

    /// Makes a group with the user as its owner and waits until the chat list shows it.
    pub fn create_group(&self, name: &str) -> Result<Group> {
        let Answer::Group(g) = self.ask(Want::NewGroup, Request::NewGroup(name.to_owned()))? else {
            return Err(unexpected(RequestKind::Group));
        };
        Ok(Group::new(self.clone(), g))
    }

    /// The friend `id`, or one known only by its id when the list does not have it yet.
    pub(super) fn friend_or_id(&self, id: i64) -> Friend {
        self.friend(id).unwrap_or_else(|| {
            let f = wire::Friend {
                id,
                ..wire::Friend::default()
            };
            Friend::new(self.clone(), f)
        })
    }

    pub(super) fn group_or_id(&self, id: i64) -> Group {
        self.group(id).unwrap_or_else(|| {
            let g = wire::ChatGroup {
                id,
                ..wire::ChatGroup::default()
            };
            Group::new(self.clone(), g)
        })
    }

    fn roster(&self) -> MutexGuard<'_, Roster> {
        let r = self.inner.shared.roster.lock();
        r.unwrap_or_else(PoisonError::into_inner)
    }
}
