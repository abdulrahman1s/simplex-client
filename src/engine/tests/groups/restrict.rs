//! A moderator's block for everyone (Subscriber.hs `xGrpMemRestrict`; Types.hs `memberBlocked`).

use crate::types::Event;
use crate::types::model::{ChatNotice, MessageBody};

use super::messages::{member_id, one, says};
use crate::chat::ChatEvent;
use crate::chat::container::MsgContainer;
use crate::chat::group::{GroupMemberRole, MemberRestrictionStatus, MemberRestrictions};
use crate::chat::ids::MemberId;
use crate::engine::tests::{T, messages};

fn restrict(member_id: MemberId, blocked: bool) -> ChatEvent {
    let restriction = if blocked {
        MemberRestrictionStatus::Blocked
    } else {
        MemberRestrictionStatus::Unrestricted
    };
    ChatEvent::GrpMemRestrict {
        member_id,
        restrictions: MemberRestrictions { restriction },
    }
}

fn notices(out: &[Event]) -> Vec<ChatNotice> {
    messages(out)
        .into_iter()
        .filter_map(|m| match m.body {
            MessageBody::Notice(n) => Some(n),
            _ => None,
        })
        .collect()
}

fn set_role(t: &mut T, g: i64, member: i64, role: GroupMemberRole) {
    let mut m = t.e.member(g, member).unwrap();
    m.role = role;
    t.e.save_member(m).unwrap();
}

/// The user is a plain member; bob is the owner who invited them, carol a member.
#[test]
fn a_block_for_all_hides_the_member_says_so_and_leaves_the_users_own_block() {
    let mut t = T::started();
    let (g, ms) = t.group("team", &["bob", "carol"], GroupMemberRole::Member);
    let (bob, carol) = (ms[0], ms[1]);
    let out = t.peer_says(&bob.1, 1, restrict(member_id(&t, g, carol.0), true));
    let blocked = ChatNotice::MemberBlocked {
        by: Some("bob".into()),
        who: "carol".into(),
        blocked: true,
    };
    assert_eq!(notices(&out), [blocked]);
    let c = t.e.member(g, carol.0).unwrap();
    assert!(c.blocked_by_admin && !c.blocked);
    let m = one(&says(&mut t, &carol.1, 1, MsgContainer::text("spam")));
    assert!(!m.unread, "a member blocked for everyone is never unread");
    t.e.set_member_blocked(g, carol.0, false).unwrap();
    assert!(
        t.e.member(g, carol.0).unwrap().hidden(),
        "the user's unblock leaves it"
    );
}

#[test]
fn an_unblock_for_all_shows_the_member_again() {
    let mut t = T::started();
    let (g, ms) = t.group("team", &["bob", "carol"], GroupMemberRole::Member);
    let (bob, carol) = (ms[0], ms[1]);
    let carol_id = member_id(&t, g, carol.0);
    t.peer_says(&bob.1, 1, restrict(carol_id.clone(), true));
    assert!(notices(&t.peer_says(&bob.1, 2, restrict(carol_id.clone(), true))).is_empty());
    let out = t.peer_says(&bob.1, 3, restrict(carol_id, false));
    assert!(matches!(
        notices(&out)[..],
        [ChatNotice::MemberBlocked { blocked: false, .. }]
    ));
    assert!(!t.e.member(g, carol.0).unwrap().hidden());
    assert!(one(&says(&mut t, &carol.1, 1, MsgContainer::text("hi"))).unread);
}

#[test]
fn a_block_for_all_needs_a_moderator_not_below_the_member() {
    let mut t = T::started();
    let (g, ms) = t.group("team", &["bob", "carol", "dave"], GroupMemberRole::Member);
    let (bob, carol, dave) = (ms[0], ms[1], ms[2]);
    t.peer_says(&carol.1, 1, restrict(member_id(&t, g, dave.0), true));
    assert!(
        !t.e.member(g, dave.0).unwrap().blocked_by_admin,
        "a member may not"
    );
    set_role(&mut t, g, carol.0, GroupMemberRole::Moderator);
    t.peer_says(&carol.1, 2, restrict(member_id(&t, g, bob.0), true));
    assert!(
        !t.e.member(g, bob.0).unwrap().blocked_by_admin,
        "nor a moderator, an owner"
    );
    t.peer_says(&carol.1, 3, restrict(member_id(&t, g, dave.0), true));
    assert!(t.e.member(g, dave.0).unwrap().blocked_by_admin);
}

#[test]
fn a_block_for_all_of_the_user_or_of_someone_unknown() {
    let mut t = T::started();
    let (g, ms) = t.group("team", &["bob"], GroupMemberRole::Member);
    let me = t.e.group(g).unwrap().my_member_id;
    assert!(notices(&t.peer_says(&ms[0].1, 1, restrict(me, true))).is_empty());
    let stranger = MemberId::mint().unwrap();
    t.peer_says(&ms[0].1, 2, restrict(stranger.clone(), true));
    let m = t.e.member_by_id(g, &stranger).cloned().unwrap();
    assert!(m.blocked_by_admin, "an unknown member is added, blocked");
}

#[test]
fn a_member_blocked_for_everyone_is_left_out_of_the_history() {
    use super::history::{dave_accepted, history, team};
    use crate::engine::agent_port::AgentEvent;
    let mut t = T::started();
    let (g, carol_conn) = team(&mut t, true);
    says(&mut t, &carol_conn, 1, MsgContainer::text("from carol"));
    let carol = t.e.state.members_of(g).next().unwrap().id;
    let mut c = t.e.member(g, carol).unwrap();
    c.blocked_by_admin = true;
    t.e.save_member(c).unwrap();
    let conn = dave_accepted(&mut t, g);
    t.agent(AgentEvent::Con { conn });
    assert!(history(&t, conn).is_empty());
}
