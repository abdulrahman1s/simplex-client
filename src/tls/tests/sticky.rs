//! A failed record ends the blocking connection for good, as it ends the sans-I/O one (RFC 8446 sections 5.2, 6).
use std::io::{Read, Write};

use super::{connect, start};
use crate::tls::TlsClient;
use crate::tls::testing::MockOpts;

fn forged_then_genuine() -> MockOpts {
    MockOpts {
        send_data: b"first".to_vec(),
        corrupt_app_record: true,
        second_data: b"second".to_vec(),
        echo: true,
        ..MockOpts::default()
    }
}

#[test]
fn blocking_reader_stays_failed_after_bad_record_mac() {
    let (client, _server) = start(forged_then_genuine(), 0);
    let mut tls = connect(client).unwrap();
    let mut b = [0u8; 6];
    assert!(
        tls.read(&mut b)
            .unwrap_err()
            .to_string()
            .contains("bad_record_mac")
    );
    assert!(
        tls.read(&mut b).is_err(),
        "the record after a forged one is never read"
    );
    assert!(
        tls.write_all(b"x").is_err(),
        "the writer refuses once the reader failed"
    );
}

#[test]
fn split_writer_refuses_after_the_reader_failed() {
    let (client, _server) = start(forged_then_genuine(), 0);
    let (mut reader, mut writer) = connect(client).unwrap().split();
    let mut b = [0u8; 6];
    assert!(reader.read(&mut b).is_err());
    assert!(writer.write_all(b"x").is_err());
    assert!(writer.request_key_update().is_err());
}

#[test]
fn sans_io_client_stays_failed_after_bad_record_mac() {
    let (mut sock, _server) = start(forged_then_genuine(), 0);
    let (mut tls, hello) = TlsClient::start(&["smp/1"]).unwrap();
    sock.write_all(&hello).unwrap();
    let mut failed = false;
    let mut buf = [0u8; 65536];
    while let Ok(n) = sock.read_within(&mut buf, std::time::Duration::from_millis(300)) {
        if n == 0 {
            break;
        }
        failed |= tls.feed(&buf[..n]).is_err();
        while let Some(o) = tls.take_outgoing() {
            sock.write_all(&o).unwrap();
        }
    }
    assert!(failed);
    assert!(tls.read_app().is_empty());
}
