//! What a switch tells the chat: `SWITCH QDSnd` started at QADD and completed once we send to the new queue, never for one given up (Agent.hs `qAddMsg`, the `AM_QTEST_` and `AM_QEND_` outcomes).
use super::rotation_peer::Peer;
use super::rotation_support::{answer_qkey, assert_v7, switch_of, wait_settled, waiting_quse};
use super::support::{WAIT, connect, rig, v7_agent};
use crate::agent::client::{AgentEvent, ConnId, SwitchPhase};
use crate::agent::testing::{Node, pump};

fn phases(a: &Node, ca: ConnId) -> Vec<SwitchPhase> {
    let e = a.events.iter();
    e.filter_map(|e| match e {
        AgentEvent::Switch { conn, phase } if *conn == ca => Some(*phase),
        _ => None,
    })
    .collect()
}

#[test]
fn a_v8_switch_is_told_started_at_qadd_and_completed_once() {
    let r = rig();
    let (mut a, mut b) = (r.node("a"), r.node("b"));
    let (ca, cb) = connect(&mut a, &mut b, false, true);
    let p = Peer::start(&r, &mut a, &mut b, cb);
    p.qadd(&mut b);
    pump(&mut [&mut a, &mut b], WAIT, |n| {
        switch_of(n[0], ca).is_some().then_some(())
    });
    assert_eq!(phases(&a, ca), [SwitchPhase::Started]);
    wait_settled(&mut a, &mut b, ca);
    assert_eq!(
        phases(&a, ca),
        [SwitchPhase::Started, SwitchPhase::Completed]
    );
}

#[test]
fn a_v7_switch_is_told_completed_once_qtest_went() {
    let r = rig();
    r.pin("b", v7_agent());
    let (mut a, mut b) = (r.node("a"), r.node("b"));
    let (ca, cb) = connect(&mut a, &mut b, false, true);
    assert_v7(&a, ca);
    let mut p = Peer::start(&r, &mut a, &mut b, cb);
    p.qadd(&mut b);
    pump(&mut [&mut a, &mut b], WAIT, |n| {
        waiting_quse(n[0], ca).then_some(())
    });
    assert_eq!(phases(&a, ca), [SwitchPhase::Started]);
    p.read_old(&r, &mut b);
    answer_qkey(&r.smp, &mut p, &mut b);
    wait_settled(&mut a, &mut b, ca);
    assert_eq!(
        phases(&a, ca),
        [SwitchPhase::Started, SwitchPhase::Completed]
    );
}

#[test]
fn a_switch_given_up_is_never_told_completed() {
    let r = rig();
    let (mut a, mut b) = (r.node("a"), r.node("b"));
    let (ca, cb) = connect(&mut a, &mut b, false, true);
    let p = Peer::start(&r, &mut a, &mut b, cb);
    a.agent.hold_switch = true;
    p.qadd(&mut b);
    pump(&mut [&mut a, &mut b], WAIT, |n| {
        switch_of(n[0], ca).is_some().then_some(())
    });
    let rcv_id = r.smp.lock().queues.by_snd.remove(&p.new.snd_id).unwrap();
    r.smp.lock().queues.by_rcv.remove(&rcv_id);
    a.agent.hold_switch = false;
    a.agent.kick(&mut a.store, ca);
    wait_settled(&mut a, &mut b, ca);
    assert_eq!(phases(&a, ca), [SwitchPhase::Started]);
}
