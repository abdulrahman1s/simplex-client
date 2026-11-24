//! The tunnel pool's opening: the ClientHello rides with the tunnel hello, and the relay's bytes wait for the tunnel's yes.
use super::pool_rig::{Rig, T0};
use super::session_rig::{answer_with, sub};
use super::tls_server::{address, serve};
use crate::smp::{PipeInput, Response, SmpEventKind};

#[test]
fn a_tunnel_pipe_sends_the_client_hello_with_the_tunnel_hello_before_the_reply() {
    let mut served = serve(0, answer_with(Response::Ok));
    let mut rig = Rig::new();
    rig.pool.ensure(&served.server, T0);
    let id = rig.last_pipe();
    rig.pool.on_input(id, PipeInput::Open, T0);
    assert_eq!(rig.wire.borrow().texts.len(), 1, "the tunnel hello");
    assert_eq!(rig.wire.borrow().frames.len(), 1, "the ClientHello");
    rig.pool.flush(T0);
    assert_eq!(
        rig.wire.borrow().frames.len(),
        1,
        "nothing else before the reply"
    );

    rig.pool
        .on_input(id, PipeInput::Text(r#"{"ok":true}"#.into()), T0);
    let mut seen = Vec::new();
    let up = rig.until(&mut served, id, T0, &mut seen, |e| {
        matches!(e, SmpEventKind::Up(_) | SmpEventKind::ConnectFailed(_))
    });
    assert!(matches!(up, SmpEventKind::Up(_)), "{up:?}");
    rig.pool.send(&served.server, sub(b"q1"), 7, T0).unwrap();
    let reply = rig.until(&mut served, id, T0 + 5, &mut seen, |e| {
        matches!(e, SmpEventKind::Reply { tag: 7, .. })
    });
    assert!(matches!(reply, SmpEventKind::Reply { answer: Ok(_), .. }));
    assert_eq!(served.finish(), None);
}

#[test]
fn bytes_before_the_tunnel_reply_fail_the_connect() {
    let mut rig = Rig::new();
    rig.pool.ensure(&address(&[1; 32]), T0);
    let id = rig.last_pipe();
    rig.pool.on_input(id, PipeInput::Open, T0);
    rig.pool.on_input(id, PipeInput::Binary(vec![22, 3, 3]), T0);
    let events = rig.events();
    assert!(
        matches!(events.first(), Some(SmpEventKind::ConnectFailed(_))),
        "{events:?}"
    );
    assert_eq!(rig.wire.borrow().closed, [id]);
}
