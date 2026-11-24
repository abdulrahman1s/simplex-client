//! The sans-I/O session against an SMP server behind the TLS test server, every byte moved by the test.
use super::session_rig::{Client, T0, answer_with, quiet, sub};
use super::tls_server::{Respond, address, serve};
use crate::smp::{HandshakeError, Response, SmpError, SmpEventKind};

#[test]
fn the_session_comes_up_over_pushed_tls_bytes_with_encrypted_blocks() {
    let mut c = Client::start(&quiet(), serve(0, answer_with(Response::Ok)), usize::MAX);
    let SmpEventKind::Up(info) = c.up() else {
        panic!("not up")
    };
    assert_eq!(info.version, 23);
    assert!(info.block_encrypted);
    assert_eq!(info.session_id.len(), 32);
    assert_eq!(c.s.session(), Some(info));
    assert_eq!(c.served.finish(), None);
}

#[test]
fn a_command_sent_before_up_is_answered_under_its_tag() {
    let mut c = Client::start(&quiet(), serve(0, answer_with(Response::Ok)), usize::MAX);
    c.s.send(sub(b"q1"), 7, T0).unwrap();
    assert!(matches!(c.up(), SmpEventKind::Up(_)));
    assert_eq!(c.reply(T0 + 10, 7), Ok(Response::Ok));
    let seen = c.served.seen.lock().unwrap().clone();
    assert_eq!(seen[0].command, b"SUB");
    assert_eq!(seen[0].entity_id, b"q1");
    assert!(
        !seen[0].auth.is_empty(),
        "SUB is signed against the session"
    );
    assert_eq!(c.served.finish(), None);
}

#[test]
fn a_server_push_arrives_as_an_event_before_the_answer() {
    let respond: Respond = Box::new(|t| {
        let msg = Response::Msg {
            msg_id: vec![1; 24],
            body: vec![5, 6],
        };
        vec![
            (Vec::new(), b"q2".to_vec(), Response::End),
            (t.corr_id.clone(), t.entity_id.clone(), msg),
        ]
    });
    let mut c = Client::start(&quiet(), serve(0, respond), usize::MAX);
    c.up();
    c.s.send(sub(b"q1"), 3, T0).unwrap();
    let answer = c.reply(T0 + 10, 3);
    assert!(matches!(answer, Ok(Response::Msg { .. })));
    let pushed = vec![
        SmpEventKind::End {
            entity_id: b"q2".to_vec(),
        },
        SmpEventKind::Msg {
            entity_id: b"q1".to_vec(),
            msg_id: vec![1; 24],
            body: vec![5, 6],
        },
    ];
    assert_eq!(c.events, pushed);
    assert_eq!(c.served.finish(), None);
}

#[test]
fn a_server_whose_identity_is_not_the_address_fails_the_handshake() {
    let mut served = serve(0, answer_with(Response::Ok));
    served.server = address(&[9; 32]);
    let mut c = Client::start(&quiet(), served, usize::MAX);
    c.s.send(sub(b"q"), 1, T0).unwrap();
    let got = c.up();
    assert_eq!(
        got,
        SmpEventKind::ConnectFailed(SmpError::Handshake(HandshakeError::Identity))
    );
    assert_eq!(
        c.events,
        [SmpEventKind::Reply {
            tag: 1,
            answer: Err(SmpError::NotConnected)
        }]
    );
    assert_eq!(c.s.next_deadline(), None);
    assert_eq!(c.s.send(sub(b"q"), 2, T0), Err(SmpError::NotConnected));
    c.served.finish();
}

#[test]
fn bytes_fed_in_small_chunks_still_make_whole_blocks() {
    let mut c = Client::start(&quiet(), serve(7, answer_with(Response::Ok)), 3);
    c.s.send(sub(b"q"), 5, T0).unwrap();
    assert!(matches!(c.up(), SmpEventKind::Up(_)));
    assert_eq!(c.reply(T0 + 10, 5), Ok(Response::Ok));
    assert_eq!(c.served.finish(), None);
}
