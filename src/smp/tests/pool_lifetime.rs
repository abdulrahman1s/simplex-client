//! The tunnel's `1000 lifetime` close: an immediate redial only after a real lifetime, the back-off otherwise.
use super::pool_rig::{Rig, T0};
use super::session_rig::answer_with;
use super::tls_server::{address, serve};
use crate::smp::{PipeInput, Response, SmpEventKind};

fn lifetime() -> PipeInput {
    PipeInput::Closed {
        code: 1000,
        reason: "lifetime".into(),
    }
}

const HOUR: u64 = 3_600_000;

#[test]
fn the_lifetime_close_of_a_tunnel_up_for_an_hour_dials_again_at_once() {
    let mut served = serve(0, answer_with(Response::Ok));
    let mut rig = Rig::new();
    rig.pool.ensure(&served.server, T0);
    let id = rig.open_tunnel(T0);
    rig.until(&mut served, id, T0, &mut Vec::new(), |e| {
        matches!(e, SmpEventKind::Up(_))
    });
    rig.pool.on_input(id, lifetime(), T0 + HOUR);
    assert_eq!(rig.pool.next_deadline(), Some(T0 + HOUR));
    rig.pool.tick(T0 + HOUR);
    assert_eq!(rig.wire.borrow().dialled.len(), 2);
}

#[test]
fn early_lifetime_closes_wait_out_the_back_off() {
    let mut rig = Rig::new();
    rig.pool.ensure(&address(&[1; 32]), T0);
    let mut now = T0;
    let mut waits = Vec::new();
    for _ in 0..3 {
        let id = rig.open_tunnel(now);
        rig.pool.on_input(id, lifetime(), now + 1);
        let next = rig.pool.next_deadline().unwrap();
        waits.push(next - (now + 1));
        now = next;
        rig.pool.tick(now);
    }
    assert_eq!(
        waits,
        [2_000, 3_000, 4_500],
        "a hostile tunnel cannot force redials"
    );
    assert_eq!(rig.wire.borrow().dialled.len(), 4);
}

#[test]
fn a_lifetime_close_soon_after_up_backs_off() {
    let mut served = serve(0, answer_with(Response::Ok));
    let mut rig = Rig::new();
    rig.pool.ensure(&served.server, T0);
    let id = rig.open_tunnel(T0);
    rig.until(&mut served, id, T0, &mut Vec::new(), |e| {
        matches!(e, SmpEventKind::Up(_))
    });
    rig.pool.on_input(id, lifetime(), T0 + 60_000);
    assert_eq!(rig.pool.next_deadline(), Some(T0 + 62_000));
}
