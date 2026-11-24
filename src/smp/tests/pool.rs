//! The tunnel pool: the tunnel hello, the session over its frames, and what each kind of ending does.
use super::pool_rig::{Rig, T0};
use super::session_rig::{answer_with, sub};
use super::tls_server::{address, serve};
use crate::smp::{MAX_FRAME, MAX_PIPES, PipeInput, Response, SmpError, SmpEventKind};

fn is_down(e: &SmpEventKind) -> bool {
    matches!(e, SmpEventKind::Down(_))
}

fn is_failed(e: &SmpEventKind) -> bool {
    matches!(e, SmpEventKind::ConnectFailed(_))
}

#[test]
fn a_relay_comes_up_through_the_tunnel_and_answers_a_tagged_command() {
    let mut served = serve(0, answer_with(Response::Ok));
    let mut rig = Rig::new();
    rig.pool.ensure(&served.server, T0);
    let id = rig.open_tunnel(T0);
    assert_eq!(
        rig.wire.borrow().texts,
        [r#"{"host":"fake.test","port":5223,"token":"tok"}"#]
    );
    let mut seen = Vec::new();
    let up = rig.until(&mut served, id, T0, &mut seen, |e| {
        matches!(e, SmpEventKind::Up(_) | SmpEventKind::ConnectFailed(_))
    });
    assert!(matches!(up, SmpEventKind::Up(_)), "{up:?}");
    assert!(rig.pool.is_up(&served.server));
    rig.pool.send(&served.server, sub(b"q1"), 7, T0).unwrap();
    let reply = rig.until(&mut served, id, T0 + 5, &mut seen, |e| {
        matches!(e, SmpEventKind::Reply { tag: 7, .. })
    });
    assert_eq!(
        reply,
        SmpEventKind::Reply {
            tag: 7,
            answer: Ok(Response::Ok)
        }
    );
    assert!(rig.wire.borrow().largest_frame <= MAX_FRAME);
    assert_eq!(served.finish(), None);
}

#[test]
fn a_command_before_the_session_is_up_is_refused_as_not_connected() {
    let mut rig = Rig::new();
    let server = address(&[1; 32]);
    let r = rig.pool.send(&server, sub(b"q"), 1, T0);
    assert_eq!(r, Err(SmpError::NotConnected));
    assert_eq!(
        rig.wire.borrow().dialled.len(),
        1,
        "the send dials the relay"
    );
}

#[test]
fn a_refused_tunnel_fails_the_connect_says_down_once_and_backs_off() {
    let mut rig = Rig::new();
    let server = address(&[1; 32]);
    rig.pool.ensure(&server, T0);
    let id = rig.last_pipe();
    rig.pool.on_input(id, PipeInput::Open, T0);
    let refused = PipeInput::Text(r#"{"ok":false,"error":"connect_failed"}"#.into());
    rig.pool.on_input(id, refused.clone(), T0);
    let events = rig.events();
    assert!(is_failed(&events[0]) && is_down(&events[1]), "{events:?}");
    assert_eq!(rig.wire.borrow().closed, [id]);
    assert_eq!(rig.pool.next_deadline(), Some(T0 + 2_000));

    rig.pool.tick(T0 + 1_999);
    assert_eq!(rig.wire.borrow().dialled.len(), 1);
    rig.pool.tick(T0 + 2_000);
    let again = rig.last_pipe();
    assert_ne!(again, id);
    rig.pool.on_input(again, PipeInput::Open, T0 + 2_000);
    rig.pool.on_input(again, refused, T0 + 2_000);
    let events = rig.events();
    assert_eq!(events.len(), 1, "no second Down: {events:?}");
    assert_eq!(rig.pool.next_deadline(), Some(T0 + 2_000 + 3_000));
}

#[test]
fn rate_limited_tunnels_wait_at_least_a_minute() {
    for code in ["rate_limited", "too_many_tunnels"] {
        let mut rig = Rig::new();
        rig.pool.ensure(&address(&[1; 32]), T0);
        let id = rig.last_pipe();
        let reply = format!(r#"{{"ok":false,"error":"{code}"}}"#);
        rig.pool.on_input(id, PipeInput::Text(reply), T0);
        assert_eq!(rig.pool.next_deadline(), Some(T0 + 60_000), "{code}");
    }
}

#[test]
fn the_lifetime_close_dials_again_at_once() {
    let mut rig = Rig::new();
    rig.pool.ensure(&address(&[1; 32]), T0);
    let id = rig.open_tunnel(T0);
    let close = PipeInput::Closed {
        code: 1000,
        reason: "lifetime".into(),
    };
    rig.pool.on_input(id, close, T0 + 50);
    assert_eq!(rig.pool.next_deadline(), Some(T0 + 50));
    rig.pool.tick(T0 + 50);
    assert_eq!(rig.wire.borrow().dialled.len(), 2);
}

#[test]
fn an_error_frame_while_up_takes_the_session_down_and_not_tls_is_logged() {
    let mut served = serve(0, answer_with(Response::Ok));
    let mut rig = Rig::new();
    rig.pool.ensure(&served.server, T0);
    let id = rig.open_tunnel(T0);
    let mut seen = Vec::new();
    rig.until(&mut served, id, T0, &mut seen, |e| {
        matches!(e, SmpEventKind::Up(_))
    });
    let frame = PipeInput::Text(r#"{"ok":false,"error":"not_tls"}"#.into());
    rig.pool.on_input(id, frame, T0 + 10);
    let events = rig.events();
    assert_eq!(
        events.iter().filter(|e| is_down(e)).count(),
        1,
        "{events:?}"
    );
    assert!(!events.iter().any(is_failed));
    assert_eq!(rig.pool.take_notes().len(), 1);
    assert!(!rig.pool.is_up(&served.server));
    served.finish();
}

#[test]
fn news_from_a_replaced_pipe_is_ignored() {
    let mut rig = Rig::new();
    rig.pool.ensure(&address(&[1; 32]), T0);
    let old = rig.last_pipe();
    rig.pool.on_input(
        old,
        PipeInput::Text(r#"{"ok":false,"error":"idle"}"#.into()),
        T0,
    );
    rig.events();
    rig.pool.tick(T0 + 2_000);
    let closed = PipeInput::Closed {
        code: 1008,
        reason: "idle".into(),
    };
    rig.pool.on_input(old, closed, T0 + 2_001);
    assert!(rig.events().is_empty());
    assert_eq!(rig.wire.borrow().closed, [old]);
}

#[test]
fn no_more_than_eight_tunnels_are_open_at_once() {
    let mut rig = Rig::new();
    for i in 0..=MAX_PIPES {
        let key = [u8::try_from(i).unwrap() + 1; 32];
        rig.pool.ensure(&address(&key), T0);
    }
    assert_eq!(rig.wire.borrow().dialled.len(), MAX_PIPES);
    assert_eq!(rig.pool.take_notes().len(), 1);
    rig.pool.retain(|s| s.key_hash != [1; 32]);
    rig.pool.tick(T0 + 2_000);
    assert_eq!(rig.wire.borrow().dialled.len(), MAX_PIPES + 1);
}

#[test]
fn a_tunnel_that_never_answers_fails_by_the_connect_timeout() {
    let mut rig = Rig::new();
    rig.pool.ensure(&address(&[1; 32]), T0);
    rig.pool.tick(T0 + 20_000);
    let events = rig.events();
    assert!(is_failed(&events[0]), "{events:?}");
    assert_eq!(rig.wire.borrow().closed.len(), 1);
}
