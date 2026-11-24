//! When the proxy cannot forward: the fallback setting decides; receipts and expiry are as without a proxy.
use std::time::Duration;

use super::proxy_rig::{Net, forwarded, net, send};
use super::support::{connect, next_msg, wait_for};
use crate::agent::client::{AgentEvent, ConnId, RcptStatus};
use crate::agent::error::AgentError;
use crate::agent::testing::Node;
use crate::smp::SmpError;
use crate::types::{ProxyFallback, ProxyMode};

/// Bob, connected to Alice directly, switches to the proxy while `a` takes no forwarding.
fn bob_behind_a_proxy_a_refuses(fallback: ProxyFallback) -> (Net, Node, Node, ConnId, ConnId) {
    let net = net();
    let mut alice = net.node("alice", &[&net.a], ProxyMode::Never);
    let mut bob = net.node("bob", &[&net.b], ProxyMode::Never);
    let (ca, cb) = connect(&mut alice, &mut bob, false, false);
    net.a.lock().no_forwarding = true;
    bob.agent.cfg.proxy_mode = ProxyMode::Always;
    bob.agent.cfg.proxy_fallback = fallback;
    (net, alice, bob, ca, cb)
}

#[test]
fn a_relay_the_proxy_cannot_forward_to_is_reached_directly_when_the_fallback_allows() {
    let (net, mut alice, mut bob, ca, cb) = bob_behind_a_proxy_a_refuses(ProxyFallback::Allow);
    let direct = net.a.count("SEND");
    send(&mut bob, cb, b"anyway");
    assert_eq!(next_msg(&mut [&mut alice, &mut bob], 0, ca).1, b"anyway");
    assert_eq!(net.b.count("PRXY"), 1);
    assert_eq!(net.a.count("SEND"), direct + 1);
    assert!(net.a.lock().forwarded.is_empty());
}

#[test]
fn a_relay_the_proxy_cannot_forward_to_is_not_reached_directly_when_the_fallback_prohibits() {
    let (net, mut alice, mut bob, _, cb) = bob_behind_a_proxy_a_refuses(ProxyFallback::Prohibit);
    let direct = net.a.count("SEND");
    let sent = send(&mut bob, cb, b"never directly");
    let err = wait_for(&mut [&mut alice, &mut bob], 1, |e| match e {
        AgentEvent::Mwarn { msg_id, err, .. } if *msg_id == sent => Some(err.clone()),
        _ => None,
    });
    let AgentError::Smp(SmpError::Proxy { err, .. }) = err else {
        panic!("not a proxy error: {err:?}")
    };
    assert_eq!(err.to_string(), "PROTOCOL PROXY BROKER TRANSPORT VERSION");
    for _ in 0..30 {
        alice.step(Duration::from_millis(5));
        bob.step(Duration::from_millis(5));
    }
    assert_eq!(net.a.count("SEND"), direct);
    assert!(
        !bob.events
            .iter()
            .any(|e| matches!(e, AgentEvent::Sent { .. }))
    );
}

#[test]
fn receipts_and_expiry_still_work_through_the_proxy() {
    let net = net();
    let mut alice = net.node("alice", &[&net.a], ProxyMode::Always);
    let mut bob = net.node("bob", &[&net.b], ProxyMode::Always);
    let (ca, cb) = connect(&mut alice, &mut bob, false, false);
    let sent = send(&mut alice, ca, b"hi");
    let (id, _) = next_msg(&mut [&mut alice, &mut bob], 1, cb);
    let receipts = forwarded(&net.a, "SEND");
    bob.agent
        .ack(&mut bob.store, cb, id, Some(b"info".to_vec()))
        .unwrap();
    let (msg, status) = wait_for(&mut [&mut alice, &mut bob], 0, |e| match e {
        AgentEvent::Rcvd {
            msg_id, receipt, ..
        } => Some((*msg_id, receipt.status)),
        _ => None,
    });
    assert_eq!((msg, status), (sent, RcptStatus::Ok));
    assert_eq!(
        forwarded(&net.a, "SEND"),
        receipts + 1,
        "the receipt came forwarded"
    );
    // Alice's proxy, her own relay, goes away for good.
    net.a.refuse(true);
    net.a.drop_links();
    let lost = send(&mut alice, ca, b"never");
    net.clock.advance(2 * 24 * 3600 * 1000 + 1);
    let err = wait_for(&mut [&mut alice], 0, |e| match e {
        AgentEvent::Merr { msg_id, err, .. } if *msg_id == lost => Some(err.clone()),
        _ => None,
    });
    assert_eq!(err, AgentError::Expired);
}
