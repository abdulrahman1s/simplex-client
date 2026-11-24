//! A key in the public header never picks the per-queue secret once the queue is confirmed: the relay knows the `dh=` key, not the secret.
use super::resync::{ad, settle, state, talk};
use super::support::{WAIT, connect, rig, wait_for};
use crate::agent::client::{AgentEvent, ConnId};
use crate::agent::delivery::{ClientMessage, ClientMsgEnvelope, PrivHeader, seal};
use crate::agent::error::AgentError;
use crate::agent::resync::SyncState;
use crate::agent::testing::{Node, pump};
use crate::agent::{AgentMsgEnvelope, E2E_ENC_MESSAGE_LENGTH, E2eParams};
use crate::crypto::{Alg, CbNonce, KeyPair, cb_encrypt, dh};
use crate::encoding::{Decode, Encode};
use crate::ratchet::{CURRENT_E2E_VERSION, PqSupport, new_resync_params};

/// An `AgentRatchetKey` with the attacker's own ratchet keys.
fn attackers_ratchet_key(n: &Node, c: ConnId) -> Vec<u8> {
    let (_, params) = new_resync_params(CURRENT_E2E_VERSION, PqSupport(false)).unwrap();
    AgentMsgEnvelope::RatchetKey {
        agent_version: n.agent.conn(c).unwrap().rec.agent_version,
        e2e_encryption: E2eParams::from_bytes(&params.params().to_bytes().unwrap()).unwrap(),
        info: b"R0".to_vec(),
    }
    .to_bytes()
    .unwrap()
}

/// A box for `c`'s receive queue under a fresh key of the attacker's and the queue's public `dh=`
/// key, as published in the full link; the attacker's key goes in the public header.
fn forged(n: &Node, c: ConnId, envelope: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let rq = n.agent.conn(c).unwrap().rec.rcv.clone().unwrap();
    let attacker = KeyPair::generate(Alg::X25519).unwrap();
    let secret = dh(&rq.e2e_priv.public().unwrap(), &attacker.private).unwrap();
    let nonce = CbNonce::random().unwrap();
    let msg = ClientMessage {
        header: PrivHeader::Empty,
        body: envelope.to_vec(),
    };
    let enc_body = cb_encrypt(
        &secret,
        &nonce,
        &msg.to_bytes().unwrap(),
        E2E_ENC_MESSAGE_LENGTH,
    );
    let body = ClientMsgEnvelope {
        version: rq.client_version,
        e2e_pub: Some(attacker.public),
        nonce,
        enc_body: enc_body.unwrap(),
    };
    (rq.rcv_id, body.to_bytes().unwrap())
}

fn error_on(n: &mut [&mut Node], who: usize, c: ConnId) -> AgentError {
    wait_for(n, who, |e| match e {
        AgentEvent::Err { conn, err } if *conn == c => Some(err.clone()),
        _ => None,
    })
}

#[test]
fn a_relay_cannot_resync_a_confirmed_connection_with_its_own_header_key() {
    let r = rig();
    let (mut a, mut b) = (r.node("a"), r.node("b"));
    let (ca, cb) = connect(&mut a, &mut b, false, false);
    talk(&mut a, &mut b, ca, cb);
    let before = ad(&a, ca);
    let (rcv_id, body) = forged(&a, ca, &attackers_ratchet_key(&a, ca));
    r.smp.inject(&rcv_id, body);
    let err = error_on(&mut [&mut a, &mut b], 0, ca);
    assert!(matches!(err, AgentError::Crypto(_)), "{err:?}");
    assert_eq!(state(&a, ca), SyncState::Ok);
    assert_eq!(ad(&a, ca), before, "the ratchet is the one agreed with b");
    assert!(
        !a.events
            .iter()
            .any(|e| matches!(e, AgentEvent::RatchetSync { .. }))
    );
    talk(&mut a, &mut b, ca, cb);
    // An honest re-sync between the two still works.
    a.agent.synchronize_ratchet(&mut a.store, ca, true).unwrap();
    pump(&mut [&mut a, &mut b], WAIT, |n| {
        (state(n[0], ca) == SyncState::Agreed).then_some(())
    });
    settle(&mut a, &mut b, ca, cb);
    assert_ne!(ad(&a, ca), before);
    assert_eq!(ad(&a, ca), ad(&b, cb));
    talk(&mut a, &mut b, ca, cb);
}

#[test]
fn a_repeated_confirmation_is_acked_silently_and_other_envelopes_under_a_header_key_are_refused() {
    let r = rig();
    let (mut a, mut b) = (r.node("a"), r.node("b"));
    let (ca, cb) = connect(&mut a, &mut b, false, false);
    let sq = b.agent.conn(cb).unwrap().rec.snd.clone().unwrap();
    let rcv_id = a.agent.conn(ca).unwrap().rec.rcv.clone().unwrap().rcv_id;
    let agent_version = b.agent.conn(cb).unwrap().rec.agent_version;
    let again = AgentMsgEnvelope::Confirmation {
        agent_version,
        e2e_encryption: None,
        enc_conn_info: b"sent again".to_vec(),
    };
    let acks = r.smp.count("ACK");
    r.smp.inject(
        &rcv_id,
        seal(&sq, &again.to_bytes().unwrap(), true).unwrap(),
    );
    pump(&mut [&mut a, &mut b], WAIT, |_| {
        (r.smp.count("ACK") > acks).then_some(())
    });
    assert!(!a.events.iter().any(|e| matches!(e, AgentEvent::Err { .. })));
    let msg = AgentMsgEnvelope::Message {
        agent_version,
        enc_agent_message: b"not a ratchet message".to_vec(),
    };
    r.smp
        .inject(&rcv_id, seal(&sq, &msg.to_bytes().unwrap(), true).unwrap());
    let err = error_on(&mut [&mut a, &mut b], 0, ca);
    assert_eq!(err, AgentError::Prohibited("msg: public header"));
    talk(&mut a, &mut b, ca, cb);
}

#[test]
fn a_ratchet_key_under_a_header_key_is_refused_before_the_queue_is_confirmed() {
    let r = rig();
    let (mut a, mut b) = (r.node("a"), r.node("b"));
    let ca = a
        .agent
        .create_invitation(&mut a.store, false, false)
        .unwrap();
    wait_for(&mut [&mut a, &mut b], 0, |e| {
        matches!(e, AgentEvent::Invitation { .. }).then_some(())
    });
    let (rcv_id, body) = forged(&a, ca, &attackers_ratchet_key(&a, ca));
    r.smp.inject(&rcv_id, body);
    let err = error_on(&mut [&mut a, &mut b], 0, ca);
    assert_eq!(err, AgentError::Prohibited("handshake: incorrect state"));
}
