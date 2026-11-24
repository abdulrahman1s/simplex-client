//! Forwarding through a proxy end to end: over the threaded connection and the sans-I/O session, with the destination's answers and the proxy's own.
use std::sync::{Arc, Mutex};
use std::time::Duration;

use super::connection::{WAIT, calm, rig};
use super::proxy_relay::{Dest, ProxyRelay};
use super::session_rig::{Client, T0, quiet};
use super::support::keypair;
use super::tls_server::serve;
use crate::crypto::{Alg, KeyPair};
use crate::smp::proxy::{ProxyClientError, seal, session_request};
use crate::smp::{
    Answer, Command, ErrorType, ProxiedRelay, ProxyError, Request, Response, SmpError,
};

const SND: &[u8] = b"sender-id";
const LONG: Duration = Duration::from_secs(10);

fn skey(sender: &KeyPair) -> Request {
    let cmd = Command::Skey(sender.public.clone());
    Request::new(cmd, SND.to_vec(), Some(sender.private.clone()))
}

fn send(sender: &KeyPair, body: &[u8]) -> Request {
    let cmd = Command::Send {
        notification: true,
        body: body.to_vec(),
    };
    Request::new(cmd, SND.to_vec(), Some(sender.private.clone()))
}

#[test]
fn a_message_through_a_proxy_connection_reaches_the_destination_which_sees_only_the_proxy() {
    let rig = rig(calm(), 23);
    let peer = rig.up();
    let mut relay = ProxyRelay::default();
    relay.dests.push(Dest::new(SND));
    let dest = relay.dests[0].address.clone();
    let mut call = |r: Request| -> Answer {
        let rx = rig.conn.send(r).unwrap();
        let t = peer.recv(WAIT).unwrap().remove(0);
        peer.reply(&t, relay.answer(&t));
        rx.recv_timeout(WAIT).unwrap()
    };
    let session = ProxiedRelay::from_answer(&dest, None, call(session_request(&dest, None, LONG)));
    let session = session.unwrap();
    assert_eq!(session.version, 23);
    let sender = keypair(Alg::Ed25519);
    for r in [skey(&sender), send(&sender, b"hello")] {
        let (pfwd, opener) = seal(&session, &r, LONG).unwrap();
        assert_eq!(opener.open(call(pfwd)), Ok(Ok(Response::Ok)));
    }
    let d = &relay.dests[0];
    assert_eq!(d.queues[SND].msgs, vec![b"hello".to_vec()]);
    assert_eq!(d.forwarded, vec![session.session_id.clone(); 2]);
    assert!(
        rig.peers.try_recv().is_err(),
        "the client connected only to the proxy"
    );
}

#[test]
fn the_sans_io_session_carries_a_relay_session_and_a_forwarded_command() {
    let relay = Arc::new(Mutex::new(ProxyRelay::default()));
    relay.lock().unwrap().dests.push(Dest::new(SND));
    let dest = relay.lock().unwrap().dests[0].address.clone();
    let r = Arc::clone(&relay);
    let respond = Box::new(move |t: &_| {
        let answer = r.lock().unwrap().answer(t);
        vec![(t.corr_id.clone(), t.entity_id.clone(), answer)]
    });
    let mut c = Client::start(&quiet(), serve(0, respond), usize::MAX);
    c.up();
    c.s.send(session_request(&dest, None, LONG), 1, T0).unwrap();
    let session = ProxiedRelay::from_answer(&dest, None, c.reply(T0, 1)).unwrap();
    let sender = keypair(Alg::Ed25519);
    let (pfwd, opener) = seal(&session, &send(&sender, b"hi"), LONG).unwrap();
    c.s.send(pfwd, 2, T0).unwrap();
    // No sender key yet: the queue takes the message unsigned only, so the destination refuses it.
    assert_eq!(
        opener.open(c.reply(T0, 2)),
        Err(SmpError::Server(ErrorType::Auth))
    );
    let (pfwd, opener) = seal(&session, &skey(&sender), LONG).unwrap();
    c.s.send(pfwd, 3, T0).unwrap();
    assert_eq!(opener.open(c.reply(T0, 3)), Ok(Ok(Response::Ok)));
    assert_eq!(relay.lock().unwrap().dests[0].forwarded.len(), 2);
    drop(c);
}

#[test]
fn a_relay_below_v23_is_sealed_and_answered_under_the_old_nonces() {
    let rig = rig(calm(), 23);
    let peer = rig.up();
    let mut relay = ProxyRelay::default();
    let mut d = Dest::new(SND);
    d.max_version = 22;
    relay.dests.push(d);
    let dest = relay.dests[0].address.clone();
    let mut call = |r: Request| -> Answer {
        let rx = rig.conn.send(r).unwrap();
        let t = peer.recv(WAIT).unwrap().remove(0);
        peer.reply(&t, relay.answer(&t));
        rx.recv_timeout(WAIT).unwrap()
    };
    let session = ProxiedRelay::from_answer(&dest, None, call(session_request(&dest, None, LONG)));
    let session = session.unwrap();
    assert_eq!(session.version, 22);
    let sender = keypair(Alg::Ed25519);
    let (pfwd, opener) = seal(&session, &skey(&sender), LONG).unwrap();
    let Command::Pfwd { version, .. } = &pfwd.command else {
        panic!("not PFWD")
    };
    assert_eq!(*version, 22);
    assert_eq!(opener.open(call(pfwd)), Ok(Ok(Response::Ok)));
}

#[test]
fn the_proxys_own_failures_stay_apart_from_the_destinations_answers() {
    let mut relay = ProxyRelay::default();
    relay.dests.push(Dest::new(SND));
    let session = ProxiedRelay {
        session_id: b"gone".to_vec(),
        version: 23,
        basic_auth: None,
        server_key: relay.dests[0].key.public.clone(),
    };
    let sender = keypair(Alg::Ed25519);
    let (_, opener) = seal(&session, &skey(&sender), LONG).unwrap();
    let no_session = ErrorType::Proxy(ProxyError::NoSession);
    assert_eq!(
        opener.open(Err(SmpError::Server(no_session.clone()))),
        Ok(Err(ProxyClientError::Protocol(no_session)))
    );
    assert_eq!(
        opener.open(Ok(Response::Ok)),
        Ok(Err(ProxyClientError::Unexpected("OK".into())))
    );
    assert_eq!(opener.open(Err(SmpError::Timeout)), Err(SmpError::Timeout));
}
