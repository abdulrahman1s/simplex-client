//! An XFTP server behind the TLS test server: a real identity chain, the handshake, then the commands, one connection per `serve`.
use std::thread::JoinHandle;

use super::fake_h2::{accept, request, respond};
use super::fake_xftp::{Shared, answer};
use crate::crypto::{Alg, KeyPair, PublicKey, key_hash};
use crate::encoding::{Version, VersionRange, b64_encode};
use crate::tls::TlsError;
use crate::tls::testing::{
    AppHook, MockOpts, MockResult, Pipe, ServerConn, make_cert, spawn_server,
};
use crate::xftp::{ServerHandshake, XftpServer};

fn der(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    match content.len() {
        n if n < 128 => out.push(n as u8),
        n => out.extend([0x82, (n >> 8) as u8, n as u8]),
    }
    out.extend(content);
    out
}

/// `SignedExact PubKey`: the DH key's SPKI, an algorithm and the leaf's signature over the SPKI.
fn signed_key(dh: &PublicKey, leaf: &KeyPair) -> Vec<u8> {
    let object = dh.to_der();
    let sig = leaf.private.sign(&object).unwrap();
    let alg = [0x30, 0x05, 0x06, 0x03, 0x2b, 0x65, 0x70];
    let bits = der(0x03, &[&[0u8][..], sig.as_bytes()].concat());
    der(0x30, &[object, alg.to_vec(), bits].concat())
}

/// A server identity and its state; every connection to it shares the state.
pub struct Fake {
    pub leaf: KeyPair,
    pub chain: Vec<Vec<u8>>,
    pub address: XftpServer,
    pub store: Shared,
}

impl Fake {
    pub fn new() -> Self {
        let ca = KeyPair::generate(Alg::Ed25519).unwrap();
        let leaf = KeyPair::generate(Alg::Ed25519).unwrap();
        let chain = vec![
            make_cert(&leaf.public, &ca, 2),
            make_cert(&ca.public, &ca, 1),
        ];
        let hash = b64_encode(&key_hash(&chain[1]), true);
        Self {
            leaf,
            chain,
            address: format!("xftp://{hash}@files.test:5443").parse().unwrap(),
            store: Shared::default(),
        }
    }

    /// One connection: the client's end of its pipe, and the server thread. `chunk` caps the
    /// bytes one pipe read returns (0 for no cap).
    pub fn serve(&self, chunk: usize) -> (Pipe, JoinHandle<MockResult>) {
        let (leaf, chain, store) = (self.leaf.clone(), self.chain.clone(), self.store.clone());
        let app: AppHook = Box::new(move |conn| run(conn, &chain, &leaf, &store));
        let opts = MockOpts {
            alpn: Some("xftp/1"),
            identity: Some((self.leaf.clone(), self.chain.clone())),
            app: Some(app),
            ..MockOpts::default()
        };
        let (client, server) = Pipe::pair(chunk);
        (client, spawn_server(server, opts))
    }
}

fn run(
    conn: &mut ServerConn,
    chain: &[Vec<u8>],
    leaf: &KeyPair,
    store: &Shared,
) -> Result<(), TlsError> {
    accept(conn)?;
    let dh_key = KeyPair::generate(Alg::X25519).unwrap();
    let hello = ServerHandshake {
        range: VersionRange::new(Version(1), Version(4)).unwrap(),
        session_id: conn.tls_unique.clone(),
        chain: chain.to_vec(),
        signed_key: signed_key(&dh_key.public, leaf),
        web_proof: None,
        info: None,
    };
    let Some(first) = request(conn)? else {
        return Ok(());
    };
    respond(conn, first.stream, &hello.encode_block().unwrap())?;
    let Some(ours) = request(conn)? else {
        return Ok(());
    };
    let raw = crate::crypto::unpad(&ours.body).unwrap();
    let version = u16::from_be_bytes([raw[0], raw[1]]);
    respond(conn, ours.stream, &[])?;
    while let Some(req) = request(conn)? {
        let body = answer(store, &conn.tls_unique, version, &req.body);
        respond(conn, req.stream, &body)?;
    }
    Ok(())
}
