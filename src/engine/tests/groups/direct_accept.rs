//! Accepting a group member's invitation joins its link and makes the member a friend; rejecting it asks nothing of the agent (Commands.hs `APIAcceptMemberContact`; Store/Groups.hs `updateMemberContactInvited`).

use crate::types::Request;
use crate::types::model::{ChatRef, MessageBody};

use super::direct_inv::{invite, requests, team};
use super::members_support::link;
use crate::engine::agent_port::AgentEvent;
use crate::engine::tests::{Call, T, messages, x_info};

#[test]
fn accepting_a_members_invitation_joins_its_link_without_pq_and_makes_a_friend_of_the_member() {
    let mut t = T::started();
    let (g, [_, (carol_id, carol)]) = team(&mut t, true);
    t.peer_says(&carol, 1, invite(Some("let's talk")));
    let [(req, _)] = requests(&mut t).try_into().unwrap();
    let out = t.req(Request::Accept(req));
    let joined = t.e.agent.calls.iter().find_map(|c| match c {
        Call::Join { link, .. } => Some(link.clone()),
        _ => None,
    });
    assert_eq!(joined, Some(link(3)));
    assert_eq!(t.e.agent.pq_seen.last(), Some(&("join", false)));
    assert!(requests(&mut t).is_empty());
    let friend = t.e.member(g, carol_id).unwrap().friend.unwrap();
    let f = t.e.friend(friend).unwrap();
    assert_eq!((f.name.as_str(), f.ready, f.pq), ("carol", false, false));
    assert!(
        messages(&out)
            .iter()
            .any(|m| m.chat == ChatRef::Direct(friend)
                && m.body == MessageBody::Text("let's talk".into()))
    );
    let conn = f.conn;
    t.agent(AgentEvent::Info {
        conn,
        conn_info: x_info("carol"),
    });
    t.agent(AgentEvent::Con { conn });
    assert!(t.e.friend(friend).unwrap().reachable());
}

#[test]
fn rejecting_a_members_invitation_asks_nothing_of_the_agent() {
    let mut t = T::started();
    let (_, [_, (_, carol)]) = team(&mut t, true);
    t.peer_says(&carol, 1, invite(None));
    let [(req, _)] = requests(&mut t).try_into().unwrap();
    t.req(Request::Reject(req));
    assert!(requests(&mut t).is_empty());
    assert!(
        !t.e.agent
            .calls
            .iter()
            .any(|c| matches!(c, Call::RejectRequest { .. }))
    );
}

#[test]
fn a_member_who_is_a_friend_moves_onto_the_new_connection() {
    let mut t = T::started();
    let (g, [_, (carol_id, carol)]) = team(&mut t, true);
    let (friend, old) = t.friend("carol");
    let mut m = t.e.member(g, carol_id).unwrap();
    m.friend = Some(friend);
    t.e.save_member(m).unwrap();
    t.peer_says(&carol, 1, invite(None));
    let [(req, _)] = requests(&mut t).try_into().unwrap();
    t.req(Request::Accept(req));
    let f = t.e.friend(friend).unwrap();
    assert_ne!(f.conn, old);
    assert_eq!(t.e.state.friend_of(&f.conn), Some(friend));
    assert_eq!(t.e.state.friend_of(&old), None);
    assert_eq!(t.e.state.friends.len(), 1);
}
