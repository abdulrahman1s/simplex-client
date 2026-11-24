//! Messages that come right before a key change must end their record (RFC 8446 section 5.1).
use std::io::{Read, Write};
use std::time::Duration;

use super::{connect, start};
use crate::tls::testing::MockOpts;
use crate::tls::{TlsClient, TlsError};

fn spans_key_change(e: &TlsError) -> bool {
    matches!(e, TlsError::Protocol(m) if m.contains("spans a key change"))
}

/// The sans-I/O client fed by hand until it fails or the server goes quiet.
fn sans_io_error(opts: MockOpts) -> Option<TlsError> {
    let (mut sock, _server) = start(opts, 0);
    let (mut tls, hello) = TlsClient::start(&["smp/1"]).unwrap();
    sock.write_all(&hello).unwrap();
    let mut buf = [0u8; 65536];
    while let Ok(n @ 1..) = sock.read_within(&mut buf, Duration::from_millis(300)) {
        if let Err(e) = tls.feed(&buf[..n]) {
            return Some(e);
        }
        while let Some(o) = tls.take_outgoing() {
            sock.write_all(&o).unwrap();
        }
    }
    None
}

fn plaintext_flight() -> MockOpts {
    MockOpts {
        plaintext_flight: true,
        send_data: b"hi".to_vec(),
        ..MockOpts::default()
    }
}

#[test]
fn encrypted_flight_sent_unencrypted_with_server_hello_is_refused() {
    let (client, _server) = start(plaintext_flight(), 0);
    let e = connect(client).err().unwrap();
    assert!(spans_key_change(&e), "{e}");
    assert!(spans_key_change(
        &sans_io_error(plaintext_flight()).unwrap()
    ));
}

fn ticket_after_finished() -> MockOpts {
    MockOpts {
        coalesce: true,
        ticket_after_finished: true,
        ..MockOpts::default()
    }
}

#[test]
fn message_after_server_finished_in_its_record_is_refused() {
    let (client, _server) = start(ticket_after_finished(), 0);
    let e = connect(client).err().unwrap();
    assert!(spans_key_change(&e), "{e}");
    assert!(spans_key_change(
        &sans_io_error(ticket_after_finished()).unwrap()
    ));
}

fn key_updates(n: usize) -> MockOpts {
    MockOpts {
        key_updates_in_one_record: n,
        send_data: b"after".to_vec(),
        ..MockOpts::default()
    }
}

#[test]
fn several_key_updates_in_one_record_are_refused() {
    let (client, _server) = start(key_updates(3), 0);
    let mut tls = connect(client).unwrap();
    let e = tls.read(&mut [0u8; 5]).unwrap_err();
    assert!(e.to_string().contains("spans a key change"), "{e}");
    let e = sans_io_error(key_updates(3)).unwrap();
    assert!(
        matches!(e, TlsError::Protocol(m) if m.contains("KeyUpdate")),
        "{e}"
    );
}

#[test]
fn one_key_update_alone_in_its_record_is_followed() {
    let (client, _server) = start(key_updates(1), 0);
    let mut tls = connect(client).unwrap();
    let mut b = [0u8; 5];
    tls.read_exact(&mut b).unwrap();
    assert_eq!(&b, b"after");
}
