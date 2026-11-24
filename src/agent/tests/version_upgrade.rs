//! A connection agreed at agent v7 moves to v8 once both sides speak it, from the version every message carries; a peer that speaks only v7 keeps it there (Agent.hs `updateConnVersion`, Ratchet.hs `rcDecrypt`).
use super::rotation_support::restart;
use super::support::{Rig, connect, next_msg, rig, v7_agent};
use crate::agent::client::ConnId;
use crate::agent::testing::Node;
use crate::agent::version::supported_agent_range;
use crate::encoding::Version;
use crate::ratchet::CURRENT_E2E_VERSION;

fn agent_version(n: &Node, c: ConnId) -> Version {
    n.agent.conn(c).unwrap().rec.agent_version
}

fn e2e_version(n: &Node, c: ConnId) -> Version {
    let rc = n.agent.conn(c).unwrap().ratchet.clone().unwrap();
    rc.versions().current
}

/// `a` sends `body` and `b` reads and acks it.
fn say(a: &mut Node, b: &mut Node, ca: ConnId, cb: ConnId, body: &[u8]) {
    a.agent.send(&mut a.store, ca, body.to_vec()).unwrap();
    let (id, got) = next_msg(&mut [&mut *a, &mut *b], 1, cb);
    assert_eq!(got, body);
    b.agent.ack(&mut b.store, cb, id, None).unwrap();
}

fn reopen(r: &Rig, a: Node, b: Node) -> (Node, Node) {
    (restart(r, a, "a"), restart(r, b, "b"))
}

#[test]
fn a_v7_connection_between_v8_agents_is_raised_by_the_first_message_each_way_and_stays_raised() {
    let r = rig();
    r.pin("a", v7_agent());
    r.pin("b", v7_agent());
    let (mut a, mut b) = (r.node("a"), r.node("b"));
    let (ca, cb) = connect(&mut a, &mut b, false, true);
    assert_eq!(agent_version(&a, ca), Version(7));
    // Both agents are updated to speak v8; the connection is stored as their v7 handshake left it.
    r.pin("a", supported_agent_range());
    r.pin("b", supported_agent_range());
    let (mut a, mut b) = reopen(&r, a, b);
    assert_eq!(agent_version(&b, cb), Version(7));
    say(&mut a, &mut b, ca, cb, b"one");
    assert_eq!(agent_version(&b, cb), Version(8), "a's message offers v8");
    assert_eq!(agent_version(&a, ca), Version(7), "a has heard nothing yet");
    say(&mut b, &mut a, cb, ca, b"two");
    assert_eq!(agent_version(&a, ca), Version(8));
    let (mut a, mut b) = reopen(&r, a, b);
    assert_eq!(agent_version(&a, ca), Version(8), "the raise is stored");
    assert_eq!(agent_version(&b, cb), Version(8));
    // 7.1.4 speaks only e2e v3, so the ratchet has nothing to move to.
    assert_eq!(e2e_version(&a, ca), CURRENT_E2E_VERSION);
    assert_eq!(e2e_version(&b, cb), CURRENT_E2E_VERSION);
    say(&mut a, &mut b, ca, cb, b"three");
    say(&mut b, &mut a, cb, ca, b"four");
}

#[test]
fn a_peer_that_speaks_only_v7_keeps_the_connection_at_v7() {
    let r = rig();
    r.pin("b", v7_agent());
    let (mut a, mut b) = (r.node("a"), r.node("b"));
    let (ca, cb) = connect(&mut a, &mut b, false, true);
    assert_eq!(agent_version(&a, ca), Version(7), "agreed in the handshake");
    assert_eq!(agent_version(&b, cb), Version(7));
    say(&mut a, &mut b, ca, cb, b"one");
    say(&mut b, &mut a, cb, ca, b"two");
    let (a, b) = reopen(&r, a, b);
    assert_eq!(agent_version(&a, ca), Version(7));
    assert_eq!(
        agent_version(&b, cb),
        Version(7),
        "v8 offered, v7 is all it speaks"
    );
}
