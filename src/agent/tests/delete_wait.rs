//! Deleting a connection with sends still waiting: they go first, across a restart too, then the connection is forgotten (Agent.hs `deleteConnectionsAsync'` with `waitDelivery`; AgentStore.hs `deleteConn`).
use super::support::{WAIT, connect, next_msg, rig};
use crate::agent::client::{AgentEvent, ConnId};
use crate::agent::connection::DELETE_DELIVERY_TIMEOUT_MS;
use crate::agent::records;
use crate::agent::testing::{Node, pump};
use crate::ratchet::PqEncryption;

fn send(n: &mut Node, c: ConnId, body: &[u8]) {
    n.agent
        .send(&mut n.store, c, PqEncryption::OFF, body.to_vec())
        .unwrap();
}

fn stored(n: &mut Node, c: ConnId) -> usize {
    records::conn_prefixes(c)
        .iter()
        .map(|p| n.store.keys_with_prefix(records::NS, p).unwrap().len())
        .sum()
}

fn gone(n: &mut Node, c: ConnId) -> bool {
    n.agent.conn(c).is_err() && stored(n, c) == 0
}

#[test]
fn messages_sent_just_before_a_delete_are_delivered_and_then_the_connection_goes() {
    let r = rig();
    let (mut a, mut b) = (r.node("a"), r.node("b"));
    let (ca, cb) = connect(&mut a, &mut b, false, false);
    send(&mut a, ca, b"one");
    send(&mut a, ca, b"bye");
    a.agent.delete(&mut a.store, ca).unwrap();
    assert!(a.agent.conn(ca).is_ok(), "kept while its sends wait");
    let mut n = [&mut a, &mut b];
    for want in [&b"one"[..], b"bye"] {
        let (id, body) = next_msg(&mut n, 1, cb);
        assert_eq!(body, want);
        n[1].agent.ack(&mut n[1].store, cb, id, None).unwrap();
    }
    pump(&mut n, WAIT, |n| gone(n[0], ca).then_some(()));
}

#[test]
fn a_connection_deleted_with_sends_waiting_still_sends_them_after_a_restart() {
    let r = rig();
    let (mut a, mut b) = (r.node("a"), r.node("b"));
    let (ca, cb) = connect(&mut a, &mut b, false, false);
    r.smp.refuse(true);
    r.smp.drop_links();
    a.step(std::time::Duration::from_millis(5));
    send(&mut a, ca, b"last words");
    a.agent.delete(&mut a.store, ca).unwrap();
    drop(a);
    let mut a = r.node("a");
    assert!(a.agent.conn(ca).is_ok(), "still waiting after the restart");
    r.smp.refuse(false);
    let mut n = [&mut a, &mut b];
    let (_, body) = next_msg(&mut n, 1, cb);
    assert_eq!(body, b"last words");
    pump(&mut n, WAIT, |n| gone(n[0], ca).then_some(()));
}

#[test]
fn a_deleted_connection_receives_nothing_more() {
    let r = rig();
    let (mut a, mut b) = (r.node("a"), r.node("b"));
    let (ca, cb) = connect(&mut a, &mut b, false, false);
    send(&mut a, ca, b"bye");
    a.agent.delete(&mut a.store, ca).unwrap();
    send(&mut b, cb, b"are you there?");
    let mut n = [&mut a, &mut b];
    pump(&mut n, WAIT, |n| gone(n[0], ca).then_some(()));
    let heard = a
        .events
        .iter()
        .any(|e| matches!(e, AgentEvent::Msg { conn, .. } if *conn == ca));
    assert!(!heard);
}

#[test]
fn a_deleted_connection_is_forgotten_after_the_delivery_timeout_whatever_waits() {
    let r = rig();
    let (mut a, mut b) = (r.node("a"), r.node("b"));
    let (ca, _) = connect(&mut a, &mut b, false, false);
    drop(b);
    send(&mut a, ca, b"into the void");
    a.agent.delete(&mut a.store, ca).unwrap();
    r.clock.advance(DELETE_DELIVERY_TIMEOUT_MS);
    let now = a.clock.now_ms();
    a.agent.on_tick(&mut a.store, now);
    assert!(gone(&mut a, ca));
}
