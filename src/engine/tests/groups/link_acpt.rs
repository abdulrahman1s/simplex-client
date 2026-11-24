//! `x.grp.link.acpt` acts only on a member that waits for it (stricter than 7.1.4's `xGrpLinkAcpt`).

use crate::chat::ChatEvent;
use crate::chat::group::{GroupAcceptance, GroupMemberRole};
use crate::chat::ids::MemberId;
use crate::engine::records::MemberStatus;
use crate::engine::tests::T;

fn acpt(member_id: MemberId, role: GroupMemberRole) -> ChatEvent {
    ChatEvent::GrpLinkAcpt {
        acceptance: GroupAcceptance::Accepted,
        role,
        member_id,
    }
}

/// bob owns the group, carol is a moderator, dave a member.
fn team(t: &mut T) -> (i64, Vec<(i64, crate::engine::agent_port::ConnId)>) {
    let (g, m) = t.group("team", &["bob", "carol", "dave"], GroupMemberRole::Member);
    let mut c = t.e.member(g, m[1].0).unwrap();
    c.role = GroupMemberRole::Moderator;
    t.e.save_member(c).unwrap();
    (g, m)
}

#[test]
fn a_link_acceptance_cannot_demote_a_member_who_is_not_pending() {
    let mut t = T::started();
    let (g, m) = team(&mut t);
    let bob_id = t.e.member(g, m[0].0).unwrap().member_id;
    t.peer_says(&m[1].1, 1, acpt(bob_id, GroupMemberRole::Observer));
    assert_eq!(t.e.member(g, m[0].0).unwrap().role, GroupMemberRole::Owner);
}

#[test]
fn a_link_acceptance_cannot_bring_a_removed_member_back() {
    let mut t = T::started();
    let (g, m) = team(&mut t);
    let mut d = t.e.member(g, m[2].0).unwrap();
    d.status = MemberStatus::Removed;
    let dave_id = d.member_id.clone();
    t.e.save_member(d).unwrap();
    t.peer_says(&m[1].1, 1, acpt(dave_id, GroupMemberRole::Member));
    assert_eq!(t.e.member(g, m[2].0).unwrap().status, MemberStatus::Removed);
}

#[test]
fn a_link_acceptance_admits_a_member_waiting_for_review() {
    let mut t = T::started();
    let (g, m) = team(&mut t);
    let mut d = t.e.member(g, m[2].0).unwrap();
    d.status = MemberStatus::PendingReview;
    let dave_id = d.member_id.clone();
    t.e.save_member(d).unwrap();
    t.peer_says(&m[1].1, 1, acpt(dave_id, GroupMemberRole::Member));
    let d = t.e.member(g, m[2].0).unwrap();
    assert!(
        d.status.current() && d.status != MemberStatus::PendingReview,
        "{:?}",
        d.status
    );
}
