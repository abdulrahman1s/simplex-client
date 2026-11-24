//! The friends, groups and requests the last chat list held, and what changed since.

use std::collections::{BTreeMap, BTreeSet};

use super::Note;
use crate::types::model::{ChatGroup, Friend, FriendRequest, Membership};

/// Bounded by the engine's chat list: every `Chats` replaces it whole.
#[derive(Debug, Default)]
pub(crate) struct Roster {
    pub(crate) friends: BTreeMap<i64, Friend>,
    pub(crate) groups: BTreeMap<i64, ChatGroup>,
    requests: BTreeSet<i64>,
    /// The first list only fills the cache: what a client already had is not news.
    seeded: bool,
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
        let mut out: Vec<Note> = requests
            .iter()
            .filter(|r| !self.requests.contains(&r.id))
            .cloned()
            .map(Note::Request)
            .collect();
        self.requests = requests.iter().map(|r| r.id).collect();

        let old = std::mem::replace(&mut self.friends, by_id(friends, |f| f.id));
        let ready = |list: &BTreeMap<i64, Friend>, id| list.get(&id).is_some_and(|f| f.ready);
        let gone = old
            .values()
            .filter(|f| f.ready && !ready(&self.friends, f.id));
        out.extend(gone.cloned().map(Note::FriendRemove));
        let added = self
            .friends
            .values()
            .filter(|f| f.ready && !ready(&old, f.id));
        out.extend(added.filter(|_| news).cloned().map(Note::FriendAdd));

        let old = std::mem::replace(&mut self.groups, by_id(groups, |g| g.id));
        let is = |list: &BTreeMap<i64, ChatGroup>, id, m| {
            list.get(&id).is_some_and(|g| g.membership == m)
        };
        for g in old.values() {
            if g.membership == Membership::Member && !is(&self.groups, g.id, Membership::Member) {
                out.push(Note::GroupRemove(g.clone()));
            }
        }
        for g in self.groups.values() {
            let was = |m| is(&old, g.id, m);
            match g.membership {
                Membership::Invited if !was(Membership::Invited) => {
                    out.push(Note::GroupInvite(g.clone()));
                }
                Membership::Member if news && !was(Membership::Member) => {
                    out.push(Note::GroupAdd(g.clone()));
                }
                Membership::Invited
                | Membership::Member
                | Membership::Joining
                | Membership::Left => {}
            }
        }
        out
    }
}

fn by_id<T: Clone>(list: &[T], id: impl Fn(&T) -> i64) -> BTreeMap<i64, T> {
    list.iter().map(|t| (id(t), t.clone())).collect()
}
