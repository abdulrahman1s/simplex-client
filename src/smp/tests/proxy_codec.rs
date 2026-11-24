//! Golden bytes of the proxy commands, responses and errors, built by hand from Protocol.hs encodings.
use super::support::*;
use crate::crypto::CbNonce;
use crate::encoding::{Decode, Decoder, Encode, Version, VersionRange};
use crate::smp::proxy::{
    BrokerError, FwdResponse, FwdTransmission, Handshake, TransportError, transmission_nonce,
};
use crate::smp::{Command, ErrorType, ProxyError, Response, SmpServer};

fn server() -> SmpServer {
    let hash = crate::encoding::b64_encode(&[1; 32], true);
    format!("smp://{hash}@smp.example.com:5223")
        .parse()
        .unwrap()
}

/// `smpEncode` of an X25519 key: its DER with a length byte.
fn key_bytes() -> Vec<u8> {
    [44u8].into_iter().chain(sample_dh_key().to_der()).collect()
}

#[test]
fn prxy_names_the_relay_as_hosts_port_and_key_hash_then_the_basic_auth() {
    // (PRXY_, ' ', ProtocolServer {host, port, keyHash}, Maybe BasicAuth)
    let mut want = b"PRXY \x01\x0fsmp.example.com\x045223\x20".to_vec();
    want.extend([1u8; 32]);
    let no_auth = Command::Prxy {
        server: server(),
        basic_auth: None,
    };
    assert_eq!(no_auth.encode(23).unwrap(), [&want[..], b"0"].concat());
    let auth = Command::Prxy {
        server: server(),
        basic_auth: Some("secret".into()),
    };
    assert_eq!(
        auth.encode(23).unwrap(),
        [&want[..], b"1\x06secret"].concat()
    );
}

#[test]
fn pfwd_carries_the_relay_version_the_command_key_and_the_sealed_tail() {
    // (PFWD_, ' ', fwdV, pubKey, Tail s)
    let pfwd = Command::Pfwd {
        version: 23,
        key: sample_dh_key(),
        body: b"sealed".to_vec(),
    };
    let want = [&b"PFWD \x00\x17"[..], &key_bytes(), b"sealed"].concat();
    assert_eq!(pfwd.encode(23).unwrap(), want);
}

fn pkey() -> Response {
    Response::Pkey {
        session_id: vec![7; 32],
        range: VersionRange::new(Version(15), Version(23)).unwrap(),
        chain: vec![b"leaf".to_vec(), b"ca".to_vec()],
        signed_key: b"signed".to_vec(),
    }
}

#[test]
fn pkey_is_the_session_id_versions_and_the_chain_with_its_signed_key() {
    // (PKEY_, ' ', sid, vr, CertChainPubKey (NonEmpty Large chain) (Large signedKey))
    let mut want = b"PKEY \x20".to_vec();
    want.extend([7u8; 32]);
    want.extend(b"\x00\x0f\x00\x17\x02\x00\x04leaf\x00\x02ca\x00\x06signed");
    assert_eq!(pkey().encode(23).unwrap(), want);
    assert_eq!(Response::decode(23, &want).unwrap(), pkey());
}

#[test]
fn pres_names_its_nonce_from_v23_and_not_before() {
    let nonce = CbNonce::new([9; 24]);
    let with = Response::Pres {
        nonce: Some(nonce),
        body: b"body".to_vec(),
    };
    let named = [&b"PRES 1"[..], &[9u8; 24], b"body"].concat();
    assert_eq!(with.encode(23).unwrap(), named);
    assert_eq!(Response::decode(23, &named).unwrap(), with);
    let without = Response::Pres {
        nonce: None,
        body: b"body".to_vec(),
    };
    assert_eq!(without.encode(23).unwrap(), b"PRES 0body");
    assert_eq!(Response::decode(23, b"PRES 0body").unwrap(), without);
    assert_eq!(without.encode(22).unwrap(), b"PRES body");
    assert_eq!(Response::decode(22, b"PRES body").unwrap(), without);
}

#[test]
fn proxy_errors_read_and_write_as_the_haskell_server_writes_them() {
    let broker = |b| ErrorType::Proxy(ProxyError::Broker(b));
    let cases: Vec<(&[u8], ErrorType)> = vec![
        (b"PROXY NO_SESSION", ErrorType::Proxy(ProxyError::NoSession)),
        (b"PROXY BASIC_AUTH", ErrorType::Proxy(ProxyError::BasicAuth)),
        (
            b"PROXY PROTOCOL AUTH",
            ErrorType::Proxy(ProxyError::Protocol(Box::new(ErrorType::Auth))),
        ),
        (
            b"PROXY BROKER TRANSPORT VERSION",
            broker(BrokerError::Transport(TransportError::Version)),
        ),
        (
            b"PROXY BROKER TRANSPORT HANDSHAKE IDENTITY",
            broker(BrokerError::Transport(TransportError::Handshake(
                Handshake::Identity,
            ))),
        ),
        // RESPONSE and UNEXPECTED carry a length-prefixed string.
        (
            b"PROXY BROKER RESPONSE \x05bad x",
            broker(BrokerError::Response("bad x".into())),
        ),
        (b"PROXY BROKER TIMEOUT", broker(BrokerError::Timeout)),
        (b"PROXY BROKER HOST", broker(BrokerError::Host)),
        (b"PROXY BROKER NETWORK", broker(BrokerError::Network)),
    ];
    for (wire, e) in cases {
        assert_eq!(ErrorType::parse(wire).unwrap(), e, "{wire:?}");
        let r = Response::Err(e);
        assert_eq!(r.encode(23).unwrap(), [&b"ERR "[..], wire].concat());
    }
    // A detail newer servers may add after NETWORK is read past.
    assert_eq!(
        ErrorType::parse(b"PROXY BROKER NETWORK FAILED").unwrap(),
        broker(BrokerError::Network)
    );
    assert!(ErrorType::parse(b"PROXY ELSEWHERE").is_err());
}

#[test]
fn a_forwarded_transmission_is_the_corr_id_version_key_and_sealed_tail() {
    // FwdTransmission: (fwdCorrId as ByteString, fwdVersion, fwdKey, Tail)
    let fwd = FwdTransmission {
        corr_id: vec![5; 24],
        version: 23,
        key: sample_dh_key(),
        body: b"x".to_vec(),
    };
    let mut out = Vec::new();
    fwd.encode(&mut out).unwrap();
    let want = [&[24u8][..], &[5; 24], b"\x00\x17", &key_bytes(), b"x"].concat();
    assert_eq!(out, want);
    assert_eq!(
        FwdTransmission::decode(&mut Decoder::new(&want)).unwrap(),
        fwd
    );
    let resp = FwdResponse {
        corr_id: vec![5; 24],
        body: b"r".to_vec(),
    };
    let mut out = Vec::new();
    resp.encode(&mut out).unwrap();
    assert_eq!(out, [&[24u8][..], &[5; 24], b"r"].concat());
    assert_eq!(FwdResponse::decode(&mut Decoder::new(&out)).unwrap(), resp);
}

#[test]
fn the_forwarded_nonce_takes_the_version_from_v23_only() {
    let corr = CbNonce::new([0xa0; 24]);
    assert_eq!(transmission_nonce(22, &corr), corr);
    let mut want = [0xa0; 24];
    want[1] ^= 23;
    assert_eq!(transmission_nonce(23, &corr), CbNonce::new(want));
}
