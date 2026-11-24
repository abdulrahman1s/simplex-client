//! Joins through a group link that never connect are capped; the oldest goes first (stricter than 7.1.4).

use super::links::made;
use super::links_host::{asks, contact};
use crate::chat::group::GroupMemberRole;
use crate::engine::groups::link_cap::MAX_LINK_JOINS;
use crate::engine::records::MemberStatus;
use crate::engine::tests::{Call, T};

#[test]
fn unconnected_link_joins_are_capped_and_the_oldest_is_dropped() {
    let mut t = T::started();
    let (g, _) = t.group("team", &["bob"], GroupMemberRole::Owner);
    let link = made(&mut t, g);
    for n in 0..=MAX_LINK_JOINS {
        asks(&mut t, link, contact(&format!("j{n}")));
    }
    let waiting: Vec<String> = (t.e.state.members_of(g))
        .filter(|m| m.via_link && m.status == MemberStatus::Accepted)
        .map(|m| m.profile.display_name.clone())
        .collect();
    assert_eq!(waiting.len(), MAX_LINK_JOINS);
    assert!(!waiting.contains(&"j0".to_owned()), "the oldest went");
    assert!(waiting.contains(&format!("j{MAX_LINK_JOINS}")));
    let deleted =
        t.e.agent
            .calls
            .iter()
            .filter(|c| matches!(c, Call::Delete { .. }));
    assert_eq!(deleted.count(), 1, "its connection too");
}
