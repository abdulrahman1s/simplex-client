//! Server certificates must be valid at the configured wall clock, in both drivers (Transport/Shared.hs `x509validate`).
use std::sync::Arc;

use super::session_rig::{Client, answer_with, quiet};
use super::tls_server::{mock, serve};
use crate::encoding::b64_encode;
use crate::smp::{
    Closer, ConnConfig, Connector, HandshakeError, Response, SmpError, SmpEventKind, SmpServer,
    TlsConnector,
};
use crate::tls::testing::tls_server;

/// 10000-01-01: past the test certificates' notAfter.
fn after_expiry() -> i64 {
    253_402_300_800
}

/// 1969-12-31: before the test certificates' notBefore.
fn before_issue() -> i64 {
    -86_400
}

fn at(wall_clock: fn() -> i64) -> ConnConfig {
    ConnConfig {
        wall_clock,
        ..quiet()
    }
}

fn blocking(cfg: &ConnConfig) -> Result<(), SmpError> {
    let (opts, fake, _) = mock(answer_with(Response::Ok));
    let port = tls_server(opts);
    let key = b64_encode(&fake.key_hash, true);
    let server: SmpServer = format!("smp://{key}@127.0.0.1:{port}").parse().unwrap();
    let slot = |_: Arc<dyn Closer>| {};
    TlsConnector
        .connect(&server, "127.0.0.1", cfg, &slot)
        .map(drop)
}

fn sans_io(cfg: &ConnConfig) -> SmpEventKind {
    Client::start(cfg, serve(0, answer_with(Response::Ok)), 8192).up()
}

const IDENTITY: SmpError = SmpError::Handshake(HandshakeError::Identity);

#[test]
fn blocking_connect_refuses_expired_and_not_yet_valid_certificates() {
    assert_eq!(blocking(&quiet()), Ok(()));
    assert_eq!(blocking(&at(after_expiry)), Err(IDENTITY));
    assert_eq!(blocking(&at(before_issue)), Err(IDENTITY));
}

#[test]
fn sans_io_session_refuses_expired_and_not_yet_valid_certificates() {
    assert!(matches!(sans_io(&quiet()), SmpEventKind::Up(_)));
    for clock in [after_expiry as fn() -> i64, before_issue] {
        let e = sans_io(&at(clock));
        assert!(matches!(e, SmpEventKind::ConnectFailed(IDENTITY)), "{e:?}");
    }
}
