//! A pinned handshake checks the chain on receipt, before the client Finished, as Transport/Client.hs `onServerCertificate` does.
use std::io::Write;
use std::time::Duration;

use super::{connect_pinned, start};
use crate::tls::testing::MockOpts;
use crate::tls::{IdentityError, Pin, TlsClient, TlsError};

fn pin() -> Pin {
    Pin {
        key_hash: vec![7; 32],
        now: 1_795_478_400,
    }
}

#[test]
fn pinned_handshake_refuses_another_identity_before_sending_finished() {
    let (mut sock, _server) = start(MockOpts::default(), 0);
    let (mut tls, hello) = TlsClient::start_pinned(&["smp/1"], pin()).unwrap();
    sock.write_all(&hello).unwrap();
    let mut buf = [0u8; 65536];
    let failed = loop {
        let n = sock.read_within(&mut buf, Duration::from_secs(5)).unwrap();
        if let Err(e) = tls.feed(&buf[..n]) {
            break e;
        }
    };
    assert!(
        matches!(failed, TlsError::Identity(IdentityError::ChainShape(1))),
        "{failed}"
    );
    assert!(
        tls.take_outgoing().is_none(),
        "no Finished goes to an unpinned server"
    );
}

#[test]
fn pinned_blocking_connect_refuses_another_identity() {
    let (client, _server) = start(MockOpts::default(), 0);
    let e = connect_pinned(client, pin()).err().unwrap();
    assert!(matches!(e, TlsError::Identity(_)), "{e}");
}
