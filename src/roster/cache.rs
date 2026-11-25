//! The friends, groups and requests the chat list holds, and what changed since.

use std::collections::{BTreeMap, BTreeSet};

use super::Note;
use crate::types::model::{ChatGroup, Friend, FriendRequest, Membership};

/// Bounded by the engine's chat list: every `Chats` replaces it whole, and `ChatsChanged`
/// changes the entries it names.
#[derive(Debug, Default)]
pub(crate) struct Roster {
    pub(crate) friends: BTreeMap<i64, Friend>,
    pub(crate) groups: BTreeMap<i64, ChatGroup>,
    requests: BTreeSet<i64>,
    /// The first list only fills the cache: what a client already had is not news.
    seeded: bool,
    /// Also tell a friend or group that changed (`FriendUpdate`, `GroupUpdate`), for a client
    /// that hears changes instead of whole lists.
    pub(crate) deltas: bool,
}

/// What `ChatsChanged` says: each friend and group as it is now, and the ids of those gone.
pub(crate) struct Changed<'a> {
    pub(crate) friends: &'a [Friend],
    pub(crate) groups: &'a [ChatGroup],
    pub(crate) removed_friends: &'a [i64],
    pub(crate) removed_groups: &'a [i64],
    pub(crate) requests: Option<&'a [FriendRequest]>,
}

impl Roster {
    /// Takes a new chat list. Requests and invitations wait for an answer, so the first list
    /// announces them too; friends and groups are news only after it.
    pub(crate) fn update(
        &mut self,
        friends: &[Friend],
        groups: &[ChatGroup],
        requests: &[FriendRequest],
    ) -> Vec<Note> {
        let news = std::mem::replace(&mut self.seeded, true);
        let mut out = self.requests_now(requests);
        let old = std::mem::replace(&mut self.friends, by_id(friends, |f| f.id));
        out.extend(friend_notes(&old, &self.friends, news, self.deltas));
        let old = std::mem::replace(&mut self.groups, by_id(groups, |g| g.id));
        out.extend(group_notes(&old, &self.groups, news, self.deltas));
        out
    }

    /// Takes the changes to the list, as a whole list would tell them.
    pub(crate) fn apply(&mut self, c: &Changed<'_>) -> Vec<Note> {
        let news = std::mem::replace(&mut self.seeded, true);
        let mut out = c.requests.map(|r| self.requests_now(r)).unwrap_or_default();
        let (old, new) = swap_in(&mut self.friends, c.friends, c.removed_friends, |f| f.id);
        out.extend(friend_notes(&old, &new, news, self.deltas));
        let (old, new) = swap_in(&mut self.groups, c.groups, c.removed_groups, |g| g.id);
        out.extend(group_notes(&old, &new, news, self.deltas));
        out
    }

    fn requests_now(&mut self, requests: &[FriendRequest]) -> Vec<Note> {
        let out = requests
            .iter()
            .filter(|r| !self.requests.contains(&r.id))
            .cloned()
            .map(Note::Request)
            .collect();
        self.requests = requests.iter().map(|r| r.id).collect();
        out
    }
}

/// A friend is added when ready, removed when no longer, and changed while ready.
fn friend_notes(
    old: &BTreeMap<i64, Friend>,
    new: &BTreeMap<i64, Friend>,
    news: bool,
    deltas: bool,
) -> Vec<Note> {
    let ready = |list: &BTreeMap<i64, Friend>, id| list.get(&id).is_some_and(|f| f.ready);
    let gone = old.values().filter(|f| f.ready && !ready(new, f.id));
    let mut out: Vec<Note> = gone.cloned().map(Note::FriendRemove).collect();
    let added = new.values().filter(|f| f.ready && !ready(old, f.id));
    out.extend(added.filter(|_| news).cloned().map(Note::FriendAdd));
    if deltas && news {
        let changed = |f: &&Friend| f.ready && old.get(&f.id).is_some_and(|o| o.ready && o != *f);
        out.extend(
            new.values()
                .filter(changed)
                .cloned()
                .map(Note::FriendUpdate),
        );
    }
    out
}

/// A group is added on becoming a member, removed on leaving, and an invitation is news.
fn group_notes(
    old: &BTreeMap<i64, ChatGroup>,
    new: &BTreeMap<i64, ChatGroup>,
    news: bool,
    deltas: bool,
) -> Vec<Note> {
    let is =
        |list: &BTreeMap<i64, ChatGroup>, id, m| list.get(&id).is_some_and(|g| g.membership == m);
    let mut out = Vec::new();
    for g in old.values() {
        if g.membership == Membership::Member && !is(new, g.id, Membership::Member) {
            out.push(Note::GroupRemove(g.clone()));
        }
    }
    for g in new.values() {
        let was = |m| is(old, g.id, m);
        match g.membership {
            Membership::Invited if !was(Membership::Invited) => {
                out.push(Note::GroupInvite(g.clone()));
            }
            Membership::Member if news && !was(Membership::Member) => {
                out.push(Note::GroupAdd(g.clone()));
            }
            Membership::Member if news && deltas && old.get(&g.id) != Some(g) => {
                out.push(Note::GroupUpdate(g.clone()));
            }
            Membership::Invited | Membership::Member | Membership::Joining | Membership::Left => {}
        }
    }
    out
}

/// Puts `changed` in `cache` and takes `gone` out: what those entries were, and are now.
fn swap_in<T: Clone>(
    cache: &mut BTreeMap<i64, T>,
    changed: &[T],
    gone: &[i64],
    id: impl Fn(&T) -> i64,
) -> (BTreeMap<i64, T>, BTreeMap<i64, T>) {
    let mut old = BTreeMap::new();
    for t in changed {
        if let Some(was) = cache.insert(id(t), t.clone()) {
            old.insert(id(t), was);
        }
    }
    for g in gone {
        if let Some(was) = cache.remove(g) {
            old.insert(*g, was);
        }
    }
    (old, by_id(changed, id))
}

fn by_id<T: Clone>(list: &[T], id: impl Fn(&T) -> i64) -> BTreeMap<i64, T> {
    list.iter().map(|t| (id(t), t.clone())).collect()
}
