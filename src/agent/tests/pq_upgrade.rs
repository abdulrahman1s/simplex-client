//! A connection made without PQ takes the KEM on once chat sends with PQ on (Agent.hs `sendMessagesB_`; Ratchet.hs `rcEncryptHeader`, `pqRatchetStep`).
use super::support::{connect, connect_pq, next_msg, rig};
use crate::agent::client::ConnId;
use crate::agent::testing::Node;
use crate::ratchet::{PqEncryption, Ratchet};

fn ratchet(n: &Node, c: ConnId) -> Ratchet {
    n.agent.conn(c).unwrap().ratchet.clone().unwrap()
}

/// `from` sends with `pq` and `to` receives and acknowledges it.
fn say(n: &mut [&mut Node; 2], from: usize, conns: [ConnId; 2], pq: PqEncryption) {
    let to = 1 - from;
    let node = &mut *n[from];
    node.agent
        .send(&mut node.store, conns[from], pq, b"hi".to_vec())
        .unwrap();
    let [x, y] = n;
    let (id, _) = next_msg(&mut [&mut **x, &mut **y], to, conns[to]);
    let node = &mut *n[to];
    node.agent
        .ack(&mut node.store, conns[to], id, None)
        .unwrap();
}

#[test]
fn a_connection_without_pq_turns_the_kem_on_when_both_sides_send_with_pq() {
    let r = rig();
    let (mut a, mut b) = (r.node("a"), r.node("b"));
    let (ca, cb) = connect(&mut a, &mut b, false, false);
    assert!(!a.agent.conn(ca).unwrap().rec.pq);
    let mut n = [&mut a, &mut b];
    for _ in 0..3 {
        say(&mut n, 0, [ca, cb], PqEncryption::ON);
        say(&mut n, 1, [ca, cb], PqEncryption::ON);
    }
    assert!(a.agent.conn(ca).unwrap().rec.pq && b.agent.conn(cb).unwrap().rec.pq);
    for r in [ratchet(&a, ca), ratchet(&b, cb)] {
        assert_eq!(
            (r.pq_support().0, r.pq_sending(), r.pq_receiving()),
            (true, PqEncryption::ON, PqEncryption::ON)
        );
    }
}

#[test]
fn a_connection_without_pq_stays_without_while_sends_keep_it_off() {
    let r = rig();
    let (mut a, mut b) = (r.node("a"), r.node("b"));
    let (ca, cb) = connect(&mut a, &mut b, false, false);
    let mut n = [&mut a, &mut b];
    for _ in 0..3 {
        say(&mut n, 0, [ca, cb], PqEncryption::OFF);
        say(&mut n, 1, [ca, cb], PqEncryption::OFF);
    }
    assert!(!a.agent.conn(ca).unwrap().rec.pq);
    assert!(!ratchet(&a, ca).pq_support().0);
}

#[test]
fn the_inviter_keeps_its_pq_support_when_the_joiners_reply_has_no_kem() {
    let r = rig();
    let (mut a, mut b) = (r.node("a"), r.node("b"));
    let (ca, cb) = connect_pq(&mut a, &mut b, [true, false], false);
    assert!(a.agent.conn(ca).unwrap().rec.pq);
    assert!(
        ratchet(&a, ca).pq_support().0,
        "as initRcvRatchet_ is given it"
    );
    assert!(!ratchet(&b, cb).pq_support().0);
    let mut n = [&mut a, &mut b];
    for _ in 0..2 {
        say(&mut n, 0, [ca, cb], PqEncryption::OFF);
        say(&mut n, 1, [ca, cb], PqEncryption::OFF);
    }
}
