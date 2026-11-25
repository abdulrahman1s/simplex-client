//! Which deletions wait for the connection's sends: only those after telling the peer it goes (Commands.hs `sendDelDeleteConns`, `leaveGroupSendMsg`, `APIRemoveMembers`; Internal.hs `deleteMemberConnection'`).

use crate::types::Request;

use super::{Call, T};
use crate::chat::ChatEvent;
use crate::chat::group::GroupMemberRole;

#[test]
fn removing_a_connected_friend_waits_for_x_direct_del_and_what_was_sent_before() {
    let mut t = T::started();
    let (bob, conn) = t.friend("bob");
    t.req(Request::RemoveFriend(bob));
    assert_eq!(t.e.agent.waited, [conn]);
}

#[test]
fn a_friend_who_removed_the_user_is_forgotten_at_once() {
    let mut t = T::started();
    let (_, conn) = t.friend("bob");
    t.peer_says(&conn, 1, ChatEvent::DirectDel);
    assert!(
        t.e.agent.calls.contains(&Call::Delete { conn }),
        "its connection goes"
    );
    assert!(t.e.agent.waited.is_empty());
}

#[test]
fn leaving_a_group_and_removing_a_member_wait_but_a_member_leaving_does_not() {
    let mut t = T::started();
    let (g, ms) = t.group("team", &["bob", "carol"], GroupMemberRole::Owner);
    t.peer_says(&ms[0].1, 1, ChatEvent::GrpLeave);
    assert!(
        t.e.agent.waited.is_empty(),
        "bob left: nothing was sent to him for it"
    );
    t.req(Request::RemoveMember {
        group: g,
        member: ms[1].0,
        messages: false,
    });
    assert_eq!(t.e.agent.waited, [ms[1].1], "carol is told she was removed");
    let (g2, ms2) = t.group("crew", &["dave"], GroupMemberRole::Member);
    t.req(Request::LeaveGroup(g2));
    assert!(t.e.agent.waited.contains(&ms2[0].1));
}
