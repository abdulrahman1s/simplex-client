//! RFC 8446 rules a server's messages and records must follow, or the handshake ends.
use std::io::Read;

use super::{connect, start};
use crate::tls::testing::MockOpts;
use crate::tls::{TlsClient, TlsError};

fn rejected(opts: MockOpts) -> TlsError {
    let (client, _server) = start(opts, 0);
    connect(client).err().unwrap()
}

#[test]
fn server_hello_without_supported_versions_is_refused() {
    let e = rejected(MockOpts {
        no_supported_versions: true,
        ..MockOpts::default()
    });
    assert!(matches!(e, TlsError::Illegal("not TLS 1.3")), "{e}");
}

#[test]
fn encrypted_extensions_may_answer_only_what_was_offered() {
    let e = rejected(MockOpts {
        ee_extension: Some(0),
        ..MockOpts::default()
    });
    assert!(matches!(e, TlsError::Unsupported(_)), "{e}");
    let (client, _server) = start(
        MockOpts {
            ee_extension: Some(10),
            ..MockOpts::default()
        },
        0,
    );
    assert!(connect(client).is_ok(), "supported_groups was offered");
}

#[test]
fn certificate_request_without_signature_algorithms_is_refused() {
    let e = rejected(MockOpts {
        cert_request: true,
        cert_request_bare: true,
        ..MockOpts::default()
    });
    assert!(
        matches!(e, TlsError::Protocol(m) if m.contains("signature_algorithms")),
        "{e}"
    );
}

#[test]
fn record_with_more_than_two_to_the_fourteen_plus_one_plaintext_bytes_is_refused() {
    let (client, _server) = start(
        MockOpts {
            oversized_app: true,
            ..MockOpts::default()
        },
        0,
    );
    let mut tls = connect(client).unwrap();
    let e = tls.read(&mut [0u8; 32]).unwrap_err();
    assert!(e.to_string().contains("record_overflow"), "{e}");
}

#[test]
fn unprotected_record_longer_than_two_to_the_fourteen_is_refused() {
    let (mut tls, _) = TlsClient::start(&["smp/1"]).unwrap();
    let mut record = vec![22, 3, 3, 0x40, 0x01];
    record.resize(5 + 0x4001, 0);
    assert!(matches!(tls.feed(&record), Err(TlsError::RecordOverflow)));
}

#[test]
fn sequence_number_never_wraps() {
    use crate::tls::record::{APPLICATION_DATA, Protection};
    let mut tx = Protection::new(&[7; 32]).at_seq(u64::MAX);
    assert!(matches!(
        tx.seal(APPLICATION_DATA, b"x"),
        Err(TlsError::Internal(_))
    ));
}
