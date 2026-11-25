//! One deadline covers TCP, TLS and the SMP handshake, however slowly the peer drips bytes (Client.hs `runClient`).
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use crossbeam_channel::bounded;

use super::tls_server::address;
use crate::encoding::b64_encode;
use crate::smp::testing::identity;
use crate::smp::{
    Closer, ConnConfig, Connector, SmpConnection, SmpError, SmpEventKind, SmpServer, SmpSession,
    TlsConnector,
};
use crate::tls::testing::{AppHook, DRIP, MockOpts, dripping_server, tls_server};

const BUDGET: Duration = Duration::from_millis(600);

fn cfg() -> ConnConfig {
    ConnConfig {
        connect_timeout: BUDGET,
        reconnect_min: Duration::from_secs(30),
        ..ConnConfig::default()
    }
}

fn local(hosts: &str, port: u16) -> SmpServer {
    at_key(&[1; 32], hosts, port)
}

fn at_key(key_hash: &[u8], hosts: &str, port: u16) -> SmpServer {
    format!("smp://{}@{hosts}:{port}", b64_encode(key_hash, true))
        .parse()
        .unwrap()
}

fn connect(server: &SmpServer) -> Result<(), SmpError> {
    let slot = |_: Arc<dyn Closer>| {};
    TlsConnector
        .connect(server, &server.hosts[0], &cfg(), &slot)
        .map(drop)
}

#[test]
fn tls_handshake_dripping_ccs_records_times_out() {
    let (port, _) = dripping_server();
    let r = connect(&local("127.0.0.1", port));
    assert_eq!(r, Err(SmpError::Timeout));
}

#[test]
fn server_hello_dripping_a_byte_per_record_times_out() {
    let app: AppHook = Box::new(|conn| {
        loop {
            conn.send(&[0])?;
            thread::sleep(DRIP);
        }
    });
    let id = identity();
    let port = tls_server(MockOpts {
        identity: Some((id.leaf, id.chain)),
        app: Some(app),
        ..MockOpts::default()
    });
    let server = at_key(&id.key_hash, "127.0.0.1", port);
    assert_eq!(connect(&server), Err(SmpError::Timeout));
}

#[test]
fn stalled_host_reports_a_timeout_and_the_next_host_is_tried() {
    let (port, taken) = dripping_server();
    let (tx, events) = bounded(16);
    let server = local("127.0.0.1,127.0.0.1", port);
    let _conn = SmpConnection::open(server, cfg(), tx).unwrap();
    let first = events.recv_timeout(BUDGET * 5).unwrap().kind;
    assert!(
        matches!(first, SmpEventKind::ConnectFailed(SmpError::Timeout)),
        "{first:?}"
    );
    assert_eq!(taken.load(Ordering::Acquire), 2);
}

#[test]
fn sans_io_connect_deadline_covers_the_tls_handshake() {
    let server = address(&[1; 32]);
    let (mut session, _hello) = SmpSession::start(&server, &cfg(), 0).unwrap();
    for now in (0..600).step_by(100) {
        assert!(session.feed(&[20, 3, 3, 0, 1, 1], now).is_empty());
        assert!(session.tick(now).is_empty());
    }
    let events = session.tick(600);
    assert!(
        matches!(events[..], [SmpEventKind::ConnectFailed(SmpError::Timeout)]),
        "{events:?}"
    );
}
