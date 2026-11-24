//! A v7 switch that the connection outgrows midway: the peer started it the v7 way, so it ends with QTEST alone and no QEND (Agent.hs `updateConnVersion`, the `AM_QTEST_` outcome).
use super::rotation_peer::Peer;
use super::rotation_support::{answer_qkey, both_ways, restart, wait_settled, waiting_quse};
use super::support::{WAIT, connect, rig, v7_agent};
use crate::agent::message::AMessage;
use crate::agent::testing::pump;
use crate::agent::version::supported_agent_range;
use crate::encoding::Version;

#[test]
fn a_v7_switch_raised_to_v8_midway_still_ends_with_qtest_and_no_qend() {
    let r = rig();
    r.pin("b", v7_agent());
    let (mut a, mut b) = (r.node("a"), r.node("b"));
    let (ca, cb) = connect(&mut a, &mut b, false, true);
    let mut p = Peer::start(&r, &mut a, &mut b, cb);
    p.qadd(&mut b);
    pump(&mut [&mut a, &mut b], WAIT, |n| {
        waiting_quse(n[0], ca).then_some(())
    });
    p.read_old(&r, &mut b);
    // The peer's agent is updated: QUSE is its first message that offers v8.
    r.pin("b", supported_agent_range());
    let mut b = restart(&r, b, "b");
    answer_qkey(&r.smp, &mut p, &mut b);
    wait_settled(&mut a, &mut b, ca);
    assert_eq!(a.agent.conn(ca).unwrap().rec.agent_version, Version(8));
    p.read_new(&r, &mut b);
    p.read_old(&r, &mut b);
    assert!(
        !p.got.iter().any(|m| matches!(m, AMessage::Qend(_))),
        "{:?}",
        p.got
    );
    assert_eq!(p.got.last(), Some(&AMessage::Qtest(vec![p.new_addr()])));
    both_ways(&r, &mut a, b, &p, ca);
}
