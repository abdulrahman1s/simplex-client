//! A proxy's PKEY is checked against the destination's address as a TLS handshake would be (Client.hs `connectSMPProxiedRelay`).
use super::support::keypair;
use super::tls_server::address;
use crate::crypto::Alg;
use crate::encoding::{Version, VersionRange};
use crate::smp::proxy::{BrokerError, Handshake, TransportError};
use crate::smp::testing::{certificate, identity, signed_key};
use crate::smp::{ErrorType, ProxiedRelay, ProxyError, Response, SmpError};

fn transport(e: TransportError) -> SmpError {
    SmpError::Server(ErrorType::Proxy(ProxyError::Broker(
        BrokerError::Transport(e),
    )))
}

fn identity_error() -> SmpError {
    transport(TransportError::Handshake(Handshake::Identity))
}

fn pkey(chain: Vec<Vec<u8>>, signed_key: Vec<u8>, max: u16) -> Response {
    Response::Pkey {
        session_id: vec![3; 32],
        range: VersionRange::new(Version(15), Version(max)).unwrap(),
        chain,
        signed_key,
    }
}

#[test]
fn a_pkey_from_the_destinations_identity_gives_its_signed_key() {
    let id = identity();
    let dh = keypair(Alg::X25519);
    let answer = pkey(id.chain.clone(), signed_key(&dh.public, &id.leaf), 23);
    let relay = ProxiedRelay::from_answer(&address(&id.key_hash), Some("pw".into()), Ok(answer));
    let relay = relay.unwrap();
    assert_eq!(relay.server_key, dh.public);
    assert_eq!(
        (relay.version, relay.basic_auth.as_deref()),
        (23, Some("pw"))
    );
    assert_eq!(relay.session_id, vec![3; 32]);
}

#[test]
fn a_pkey_from_another_identity_than_the_address_pins_is_refused() {
    let (id, other) = (identity(), identity());
    let answer = pkey(
        other.chain.clone(),
        signed_key(&keypair(Alg::X25519).public, &other.leaf),
        23,
    );
    let r = ProxiedRelay::from_answer(&address(&id.key_hash), None, Ok(answer));
    assert_eq!(r, Err(identity_error()));
}

#[test]
fn a_foreign_leaf_presented_with_the_real_ca_certificate_is_refused() {
    let id = identity();
    let (mallory_ca, mallory_leaf) = (keypair(Alg::Ed25519), keypair(Alg::Ed25519));
    let forged = vec![
        certificate(&mallory_leaf.public, &mallory_ca),
        id.chain[1].clone(),
    ];
    let answer = pkey(
        forged,
        signed_key(&keypair(Alg::X25519).public, &mallory_leaf),
        23,
    );
    let r = ProxiedRelay::from_answer(&address(&id.key_hash), None, Ok(answer));
    assert_eq!(r, Err(identity_error()));
}

#[test]
fn a_key_not_signed_by_the_destinations_leaf_is_refused() {
    let id = identity();
    let answer = pkey(
        id.chain.clone(),
        signed_key(&keypair(Alg::X25519).public, &keypair(Alg::Ed25519)),
        23,
    );
    let r = ProxiedRelay::from_answer(&address(&id.key_hash), None, Ok(answer));
    assert_eq!(r, Err(identity_error()));
}

#[test]
fn a_relay_with_no_version_in_common_is_refused_as_a_version_error() {
    let id = identity();
    let mut answer = pkey(
        id.chain.clone(),
        signed_key(&keypair(Alg::X25519).public, &id.leaf),
        23,
    );
    if let Response::Pkey { range, .. } = &mut answer {
        *range = VersionRange::new(Version(24), Version(25)).unwrap();
    }
    let r = ProxiedRelay::from_answer(&address(&id.key_hash), None, Ok(answer));
    assert_eq!(r, Err(transport(TransportError::Version)));
}

#[test]
fn an_answer_to_prxy_other_than_pkey_is_unexpected() {
    let id = identity();
    let r = ProxiedRelay::from_answer(&address(&id.key_hash), None, Ok(Response::Ok));
    assert_eq!(r, Err(SmpError::Unexpected("answer to PRXY")));
}
