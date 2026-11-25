//! Every change a whole chat list shows was marked, so the `ChatsChanged` built from the marks says the same: checked by `T::out` on every list the suite sees, which it can also record.

use std::collections::{BTreeMap, BTreeSet};

use crate::types::Event;
use crate::types::model::{ChatGroup, Friend, FriendRequest};

use crate::engine::changes::Changes;

type Lists = (
    BTreeMap<i64, Friend>,
    BTreeMap<i64, ChatGroup>,
    Vec<FriendRequest>,
);

/// The last whole list `T::out` saw, and everything it put out once `record` is on.
#[derive(Default)]
pub struct Listed {
    last: Option<Lists>,
    pub record: Option<Vec<Event>>,
}

impl Listed {
    /// What differs between the last list and the next in `out` must be in `marked`.
    pub fn check(&mut self, marked: &Changes, out: &[Event]) {
        if let Some(r) = &mut self.record {
            r.extend_from_slice(out);
        }
        let next = out.iter().rev().find_map(|e| match e {
            Event::Chats {
                friends,
                groups,
                requests,
                ..
            } => Some((
                friends.iter().map(|f| (f.id, f.clone())).collect(),
                groups.iter().map(|g| (g.id, g.clone())).collect(),
                requests.clone(),
            )),
            _ => None,
        });
        let Some(next) = next else { return };
        if let Some((friends, groups, requests)) = &self.last
            && !marked.all
        {
            for id in differing(friends, &next.0) {
                assert!(
                    marked.friends.contains(&id),
                    "friend {id} changed unmarked: {:?} -> {:?}",
                    friends.get(&id),
                    next.0.get(&id)
                );
            }
            for id in differing(groups, &next.1) {
                assert!(
                    marked.groups.contains(&id),
                    "group {id} changed unmarked: {:?} -> {:?}",
                    groups.get(&id),
                    next.1.get(&id)
                );
            }
            assert!(
                *requests == next.2 || marked.requests,
                "requests changed unmarked: {requests:?} -> {:?}",
                next.2
            );
        }
        self.last = Some(next);
    }
}

fn differing<T: PartialEq>(a: &BTreeMap<i64, T>, b: &BTreeMap<i64, T>) -> BTreeSet<i64> {
    let ids = a.keys().chain(b.keys()).copied();
    ids.filter(|id| a.get(id) != b.get(id)).collect()
}
