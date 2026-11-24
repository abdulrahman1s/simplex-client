//! The sans-I/O client against the in-process server, with the test moving every byte by hand.
use std::io::{Read, Write};

use super::{opts_for, start};
use crate::crypto::Alg;
use crate::tls::testing::{MockOpts, Pipe};
use crate::tls::{TlsClient, TlsError};

fn flush(tls: &mut TlsClient, sock: &mut Pipe) {
    while let Some(out) = tls.take_outgoing() {
        sock.write_all(&out).unwrap();
    }
}

/// Reads once from the socket and feeds what came, `step` bytes per `feed`.
fn pump(tls: &mut TlsClient, sock: &mut Pipe, step: usize) -> Result<(), TlsError> {
    let mut buf = [0u8; 8192];
    let n = sock.read(&mut buf).unwrap();
    assert!(n > 0, "the server closed the connection");
    for chunk in buf[..n].chunks(step) {
        tls.feed(chunk)?;
    }
    flush(tls, sock);
    Ok(())
}

fn handshake(sock: &mut Pipe, step: usize) -> Result<TlsClient, TlsError> {
    let (mut tls, hello) = TlsClient::start(&["smp/1"])?;
    sock.write_all(&hello).unwrap();
    while !tls.is_ready() {
        pump(&mut tls, sock, step)?;
    }
    Ok(tls)
}

fn read_app(tls: &mut TlsClient, sock: &mut Pipe, len: usize, step: usize) -> Vec<u8> {
    let mut got = tls.read_app();
    while got.len() < len {
        pump(tls, sock, step).unwrap();
        got.extend(tls.read_app());
    }
    got
}

fn echo(opts: MockOpts, chunk: usize, step: usize, len: u32) {
    let (mut sock, server) = start(opts, chunk);
    let mut tls = handshake(&mut sock, step).unwrap();
    assert_eq!(tls.info().unwrap().alpn(), Some("smp/1"));
    assert_eq!(tls.peer_cert_chain().len(), 1);
    let payload: Vec<u8> = (0..len).map(|i| (i * 7) as u8).collect();
    sock.write_all(&tls.write_app(&payload).unwrap()).unwrap();
    assert_eq!(read_app(&mut tls, &mut sock, payload.len(), step), payload);
    let unique = *tls.tls_unique().unwrap();
    sock.write_all(&tls.close().unwrap()).unwrap();
    let r = server.join().unwrap();
    assert_eq!(r.error, None);
    assert_eq!(r.client_finished, unique);
    assert_eq!(r.received, payload);
}

#[test]
fn pushed_bytes_complete_the_handshake_and_carry_data_both_ways() {
    echo(opts_for(Alg::Ed25519), 0, usize::MAX, 40_000);
}

#[test]
fn bytes_fed_one_at_a_time_are_reassembled_into_records() {
    let opts = MockOpts {
        fragment: 7,
        cert_request: true,
        compat_ccs: true,
        ..opts_for(Alg::Ed448)
    };
    echo(opts, 1, 1, 3000);
}

#[test]
fn a_server_key_update_is_answered_before_our_next_record() {
    let opts = MockOpts {
        key_update: true,
        send_data: vec![1, 2, 3],
        ..opts_for(Alg::Ed25519)
    };
    let (mut sock, server) = start(opts, 0);
    let mut tls = handshake(&mut sock, usize::MAX).unwrap();
    assert_eq!(read_app(&mut tls, &mut sock, 3, usize::MAX), [1, 2, 3]);
    sock.write_all(&tls.write_app(&[9]).unwrap()).unwrap();
    assert_eq!(read_app(&mut tls, &mut sock, 1, usize::MAX), [9]);
    sock.write_all(&tls.close().unwrap()).unwrap();
    assert_eq!(server.join().unwrap().error, None);
}

#[test]
fn data_before_close_notify_is_kept_and_the_close_is_seen() {
    let opts = MockOpts {
        send_data: vec![4, 5],
        close_notify_after_data: true,
        ..MockOpts::default()
    };
    let (mut sock, _server) = start(opts, 0);
    let mut tls = handshake(&mut sock, usize::MAX).unwrap();
    while !tls.is_peer_closed() {
        pump(&mut tls, &mut sock, usize::MAX).unwrap();
    }
    assert_eq!(tls.read_app(), [4, 5]);
}

#[test]
fn a_wrong_server_finished_fails_the_pushed_handshake() {
    let opts = MockOpts {
        bad_finished: true,
        ..MockOpts::default()
    };
    let (mut sock, _server) = start(opts, 0);
    let e = handshake(&mut sock, usize::MAX).err().unwrap();
    assert!(matches!(e, TlsError::BadFinished), "{e}");
}

#[test]
fn errors_are_sticky_and_data_waits_for_the_handshake() {
    let (mut tls, _) = TlsClient::start(&["smp/1"]).unwrap();
    assert!(tls.write_app(b"early").is_err());
    assert!(tls.close().unwrap().is_empty());
    assert!(matches!(
        tls.feed(&[22, 9, 3, 0, 0]),
        Err(TlsError::Illegal(_))
    ));
    assert!(
        tls.feed(&[]).is_err(),
        "nothing is processed after a failure"
    );
}
