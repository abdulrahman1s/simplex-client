//! A proxy that hands the client's own PFWD body back as the PRES is refused before it is opened.
use std::time::Duration;

use super::support::keypair;
use crate::crypto::{Alg, CbNonce};
use crate::smp::proxy::{ProxiedRelay, seal, transmission_nonce};
use crate::smp::{Command, ProxyClientError, Request, Response};

fn reflected(nonce_of: impl Fn(u16, &CbNonce) -> CbNonce) -> ProxyClientError {
    let relay = ProxiedRelay {
        session_id: vec![9; 32],
        version: 23,
        basic_auth: None,
        server_key: keypair(Alg::X25519).public,
    };
    let send = Command::Send {
        notification: false,
        body: b"hello".to_vec(),
    };
    let (pfwd, opener) = seal(
        &relay,
        &Request::new(send, vec![1; 24], None),
        Duration::ZERO,
    )
    .unwrap();
    let Command::Pfwd { body, .. } = pfwd.command else {
        panic!("PFWD expected")
    };
    let nonce = CbNonce::from_slice(pfwd.corr_id.as_ref().unwrap()).unwrap();
    let pres = Response::Pres {
        nonce: Some(nonce_of(23, &nonce)),
        body,
    };
    opener.open(Ok(pres)).unwrap().unwrap_err()
}

fn is_reflection(e: &ProxyClientError) -> bool {
    matches!(e, ProxyClientError::Unexpected(s) if s.contains("own nonce"))
}

#[test]
fn pres_under_the_sealing_nonce_is_refused() {
    let e = reflected(transmission_nonce);
    assert!(is_reflection(&e), "{e:?}");
}

#[test]
fn pres_under_the_request_nonce_is_refused() {
    let e = reflected(|_, n| *n);
    assert!(is_reflection(&e), "{e:?}");
}
