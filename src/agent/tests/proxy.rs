//! Private message routing between two agents on two relays: through the proxy, and directly for a relay of our own.
use super::proxy_rig::{forwarded, net, send};
use super::support::{connect, next_msg};
use crate::types::ProxyMode;

#[test]
fn with_always_every_command_for_the_peers_relay_goes_through_our_own() {
    let net = net();
    let mut alice = net.node("alice", &[&net.a], ProxyMode::Always);
    let mut bob = net.node("bob", &[&net.b], ProxyMode::Always);
    let (ca, cb) = connect(&mut alice, &mut bob, false, true);
    send(&mut alice, ca, b"to bob");
    assert_eq!(next_msg(&mut [&mut alice, &mut bob], 1, cb).1, b"to bob");
    send(&mut bob, cb, b"to alice");
    assert_eq!(next_msg(&mut [&mut alice, &mut bob], 0, ca).1, b"to alice");
    for (s, peer_proxy) in [(&net.a, &net.b), (&net.b, &net.a)] {
        assert!(forwarded(s, "SEND") >= 2, "{:?}", s.lock().forwarded);
        assert_eq!(
            s.count("SEND"),
            forwarded(s, "SEND"),
            "no SEND came directly"
        );
        assert_eq!(s.count("SKEY"), forwarded(s, "SKEY"));
        assert!(peer_proxy.count("PRXY") >= 1 && peer_proxy.count("PFWD") >= 2);
        assert_eq!(
            s.lock().connects,
            1,
            "only the relay's own user connected to it"
        );
    }
    // Bob read Alice's short link through his proxy too.
    assert_eq!(forwarded(&net.a, "LKEY"), 1);
}

#[test]
fn with_unknown_a_relay_of_our_own_is_reached_directly_and_another_through_a_proxy() {
    let net = net();
    let mut alice = net.node("alice", &[&net.a], ProxyMode::Unknown);
    let mut bob = net.node("bob", &[&net.b, &net.a], ProxyMode::Unknown);
    let (ca, cb) = connect(&mut alice, &mut bob, false, false);
    send(&mut alice, ca, b"to bob");
    assert_eq!(next_msg(&mut [&mut alice, &mut bob], 1, cb).1, b"to bob");
    send(&mut bob, cb, b"to alice");
    assert_eq!(next_msg(&mut [&mut alice, &mut bob], 0, ca).1, b"to alice");
    assert!(net.a.lock().forwarded.is_empty(), "a is Bob's own relay");
    assert_eq!(net.a.lock().connects, 2);
    assert!(forwarded(&net.b, "SEND") >= 2, "b is not Alice's");
    assert_eq!(net.b.count("SEND"), forwarded(&net.b, "SEND"));
    assert_eq!(net.b.lock().connects, 1);
}

#[test]
fn a_session_the_proxy_lost_is_asked_for_again_and_the_message_goes() {
    let net = net();
    let mut alice = net.node("alice", &[&net.a], ProxyMode::Never);
    let mut bob = net.node("bob", &[&net.b], ProxyMode::Always);
    let (ca, cb) = connect(&mut alice, &mut bob, false, false);
    let asked = net.b.count("PRXY");
    net.b.lock().relayed.clear();
    send(&mut bob, cb, b"after NO_SESSION");
    assert_eq!(
        next_msg(&mut [&mut alice, &mut bob], 0, ca).1,
        b"after NO_SESSION"
    );
    assert_eq!(net.b.count("PRXY"), asked + 1);
}

#[test]
fn with_unknown_a_preset_relay_counts_as_our_own() {
    let net = net();
    let mut alice = net.node("alice", &[&net.a], ProxyMode::Unknown);
    alice.agent.cfg.presets = vec![net.b.address()];
    let mut bob = net.node("bob", &[&net.b], ProxyMode::Never);
    let (ca, cb) = connect(&mut alice, &mut bob, false, false);
    send(&mut alice, ca, b"to bob");
    assert_eq!(next_msg(&mut [&mut alice, &mut bob], 1, cb).1, b"to bob");
    assert!(net.b.lock().forwarded.is_empty());
    assert_eq!(net.a.count("PRXY"), 0);
}
