//! An invitee who waited for review gets its introductions and the history once a moderator admits it (Subscriber.hs `xGrpLinkAcpt`, `introduceToRemainingMembers`).

use super::super::T;
use super::history::{dave_accepted, history, team};
use super::members_support::sent_on;
use super::messages::says;
use crate::chat::ChatEvent;
use crate::chat::container::MsgContainer;
use crate::chat::group::{GroupAcceptance, GroupMemberRole};
use crate::engine::agent_port::{AgentEvent, ConnId};
use crate::engine::records::MemberStatus;

/// alice's invitee dave, his CONF in and held for review (the engine's own link never holds
/// anyone, so the status is set here), then connected: (group, carol's conn, dave's conn).
fn dave_waits(t: &mut T) -> (i64, ConnId, ConnId) {
    let (g, carol) = team(t, true);
    says(t, &carol, 1, MsgContainer::text("from carol"));
    let conn = dave_accepted(t, g);
    let (_, dave) = t.e.state.member_of(&conn).unwrap();
    let mut m = t.e.member(g, dave).unwrap();
    m.status = MemberStatus::PendingReview;
    t.e.save_member(m).unwrap();
    let carol_id = t.e.state.member_of(&carol).unwrap().1;
    let mut c = t.e.member(g, carol_id).unwrap();
    c.role = GroupMemberRole::Moderator;
    t.e.save_member(c).unwrap();
    t.agent(AgentEvent::Con { conn });
    (g, carol, conn)
}

fn intros(t: &T, conn: ConnId) -> usize {
    sent_on(t, conn)
        .iter()
        .filter(|e| matches!(e, ChatEvent::GrpMemIntro { .. }))
        .count()
}

#[test]
fn an_invitee_admitted_by_a_moderator_is_introduced_and_gets_the_history() {
    let mut t = T::started();
    let (g, carol, dave) = dave_waits(&mut t);
    assert_eq!((intros(&t, dave), history(&t, dave).len()), (0, 0));
    let (_, dave_member) = t.e.state.member_of(&dave).unwrap();
    let member_id = t.e.member(g, dave_member).unwrap().member_id;
    t.peer_says(
        &carol,
        2,
        ChatEvent::GrpLinkAcpt {
            acceptance: GroupAcceptance::Accepted,
            role: GroupMemberRole::Member,
            member_id,
        },
    );
    assert_eq!(intros(&t, dave), 1);
    let got = history(&t, dave);
    assert!(
        matches!(got.as_slice(), [(Some(_), ChatEvent::MsgNew(c))] if c.content.text() == "from carol"),
        "{got:?}"
    );
}
