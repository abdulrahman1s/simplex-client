//! The session's clock is the caller's: keepalive PINGs, timeouts and the connect deadline, all on `tick`.
use std::time::Duration;

use super::session_rig::{Client, T0, answer_with, quiet, sub};
use super::tls_server::{Respond, serve};
use crate::smp::{ConnConfig, Response, SmpError, SmpEventKind, SmpSession};

const PING: u64 = 30_000;
const TIMEOUT: u64 = 15_000;

fn keepalive() -> ConnConfig {
    ConnConfig {
        ping_interval: Duration::from_millis(PING),
        command_timeout: Duration::from_millis(TIMEOUT),
        max_timeouts: 2,
        ..ConnConfig::default()
    }
}

fn pings_only(r: Response) -> Respond {
    Box::new(move |t| {
        if t.command == b"PING" {
            vec![(t.corr_id.clone(), t.entity_id.clone(), r.clone())]
        } else {
            Vec::new()
        }
    })
}

#[test]
fn an_answered_ping_on_tick_keeps_the_session_up() {
    let mut c = Client::start(
        &keepalive(),
        serve(0, pings_only(Response::Pong)),
        usize::MAX,
    );
    c.up();
    assert_eq!(c.s.next_deadline(), Some(T0 + PING));
    assert!(c.s.tick(T0 + PING).is_empty());
    c.flush();
    c.server_saw(1);
    assert_eq!(c.served.seen.lock().unwrap()[0].command, b"PING");
    while c.s.next_deadline() != Some(T0 + 2 * PING) {
        c.pump(T0 + PING + 10);
    }
    assert!(c.events.is_empty(), "a PONG is nobody's answer");
    assert!(c.s.tick(T0 + PING + TIMEOUT).is_empty());
    assert!(c.s.is_up());
    assert_eq!(c.served.finish(), None);
}

#[test]
fn two_unanswered_pings_end_the_session_as_unresponsive() {
    let mut c = Client::start(&keepalive(), serve(0, Box::new(|_| Vec::new())), usize::MAX);
    c.up();
    assert!(c.s.tick(T0 + PING).is_empty());
    c.flush();
    assert!(
        c.s.tick(T0 + PING + TIMEOUT).is_empty(),
        "the first silent timeout is a strike, not the end"
    );
    assert!(c.s.tick(T0 + 2 * PING).is_empty());
    c.flush();
    let events = c.s.tick(T0 + 2 * PING + TIMEOUT);
    assert_eq!(events, [SmpEventKind::Down(SmpError::Unresponsive)]);
    assert!(!c.s.is_up());
    c.server_saw(2);
    assert_eq!(c.served.finish(), None);
}

#[test]
fn every_timed_out_command_is_answered_before_the_session_goes_down() {
    let mut c = Client::start(&keepalive(), serve(0, Box::new(|_| Vec::new())), usize::MAX);
    c.up();
    for tag in 1..=3 {
        c.s.send(sub(b"q"), tag, T0 + 10).unwrap();
    }
    c.flush();
    let events = c.s.tick(T0 + 10 + TIMEOUT);
    let mut tags: Vec<u64> = events
        .iter()
        .filter_map(|e| match e {
            SmpEventKind::Reply {
                tag,
                answer: Err(SmpError::Timeout),
            } => Some(*tag),
            _ => None,
        })
        .collect();
    tags.sort_unstable();
    assert_eq!(tags, [1, 2, 3]);
    assert_eq!(
        events.last(),
        Some(&SmpEventKind::Down(SmpError::Unresponsive))
    );
    assert_eq!(events.len(), 4);
    assert_eq!(c.served.finish(), None);
}

#[test]
fn the_handshake_must_finish_by_the_connect_deadline() {
    let cfg = ConnConfig {
        queue_capacity: 1,
        ..quiet()
    };
    let server = serve(0, answer_with(Response::Ok));
    let (mut s, _hello) = SmpSession::start(&server.server, &cfg, 0).unwrap();
    s.send(sub(b"q"), 1, 0).unwrap();
    assert_eq!(s.send(sub(b"q"), 2, 0), Err(SmpError::Busy));
    let deadline = s.next_deadline().unwrap();
    assert_eq!(deadline, 20_000);
    assert!(s.tick(deadline - 1).is_empty());
    assert_eq!(
        s.tick(deadline),
        [
            SmpEventKind::Reply {
                tag: 1,
                answer: Err(SmpError::NotConnected)
            },
            SmpEventKind::ConnectFailed(SmpError::Timeout)
        ]
    );
    server.finish();
}

#[test]
fn close_answers_what_waits_and_sends_close_notify() {
    let mut c = Client::start(&quiet(), serve(0, Box::new(|_| Vec::new())), usize::MAX);
    c.up();
    c.s.send(sub(b"q"), 4, T0).unwrap();
    let events = c.s.close();
    assert_eq!(
        events,
        [SmpEventKind::Reply {
            tag: 4,
            answer: Err(SmpError::Closed)
        }]
    );
    assert!(c.s.take_outgoing().is_some(), "close_notify");
    assert_eq!(c.s.next_deadline(), None);
    c.served.finish();
}
