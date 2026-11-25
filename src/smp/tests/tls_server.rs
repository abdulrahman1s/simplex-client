//! An SMP server behind the TLS test server: a real identity chain, the SMP handshake, block encryption, then scripted answers.
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use super::support::keypair;
use crate::crypto::{
    Alg, KeyPair, PublicKey, SbKey, dh, pad, sb_decrypt, sb_encrypt, sbc_hkdf, sbc_init, unpad,
};
use crate::encoding::{Version, VersionRange, b64_encode};
use crate::net::Socket;
use crate::smp::testing::{identity, signed_key};
use crate::smp::{
    BLOCK_SIZE, Incoming, Response, ServerHello, SmpServer, encode_transmission, pack,
    parse_payload,
};
use crate::tls::TlsError;
use crate::tls::testing::{AppHook, MockOpts, MockResult, Pipe, ServerConn, spawn_server};

/// What the server answers to one transmission: (corr id, entity id, response); an empty corr id is a push.
pub type Respond = Box<dyn FnMut(&Incoming) -> Vec<(Vec<u8>, Vec<u8>, Response)> + Send>;

pub struct Served {
    /// The client's end of the byte pipe.
    pub sock: Pipe,
    /// The address of this server: its identity's key hash.
    pub server: SmpServer,
    /// Every transmission the server received, in order.
    pub seen: Arc<Mutex<Vec<Incoming>>>,
    handle: JoinHandle<MockResult>,
}

impl Served {
    /// Cuts the pipe and returns what the server thread reported.
    pub fn finish(self) -> Option<String> {
        self.sock.shutdown(std::net::Shutdown::Both).ok();
        self.handle.join().unwrap().error
    }
}

pub fn address(key_hash: &[u8]) -> SmpServer {
    format!("smp://{}@fake.test", b64_encode(key_hash, true))
        .parse()
        .unwrap()
}

/// Starts the server; `chunk` caps the bytes one pipe read returns (0 for no cap).
pub fn serve(chunk: usize, respond: Respond) -> Served {
    let (opts, server, seen) = mock(respond);
    let (sock, server_end) = Pipe::pair(chunk);
    Served {
        sock,
        server,
        seen,
        handle: spawn_server(server_end, opts),
    }
}

/// The TLS test server's options for an SMP server, its address and what it will receive.
pub fn mock(respond: Respond) -> (MockOpts, SmpServer, Arc<Mutex<Vec<Incoming>>>) {
    let id = identity();
    let dh_key = keypair(Alg::X25519);
    let hello = ServerHello {
        range: VersionRange::new(Version(15), Version(23)).unwrap(),
        session_id: Vec::new(),
        chain: id.chain.clone(),
        signed_key: signed_key(&dh_key.public, &id.leaf),
        server_info: None,
    };
    let seen = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&seen);
    let app: AppHook = Box::new(move |conn| run(conn, hello, &dh_key, respond, &log));
    let opts = MockOpts {
        identity: Some((id.leaf, id.chain)),
        app: Some(app),
        ..MockOpts::default()
    };
    (opts, address(&id.key_hash), seen)
}

fn run(
    conn: &mut ServerConn,
    mut hello: ServerHello,
    dh_key: &KeyPair,
    mut respond: Respond,
    seen: &Mutex<Vec<Incoming>>,
) -> Result<(), TlsError> {
    hello.session_id = conn.tls_unique.clone();
    conn.send(&pad(&hello.encode().unwrap(), BLOCK_SIZE).unwrap())?;
    let client_hello = conn.recv_exact(BLOCK_SIZE)?;
    let client_hello = unpad(&client_hello).unwrap();
    let version = u16::from_be_bytes([client_hello[0], client_hello[1]]);
    let (mut snd, mut rcv) = if client_hello[35] == 44 {
        let client_key = PublicKey::from_der(&client_hello[36..80]).unwrap();
        let secret = dh(&client_key, &dh_key.private).unwrap();
        let (a, b) = sbc_init(&hello.session_id, secret.as_bytes()).unwrap();
        (Some(a), Some(b))
    } else {
        (None, None)
    };
    // The client closing the pipe ends the loop.
    while let Ok(block) = conn.recv_exact(BLOCK_SIZE) {
        for t in parse_payload(version, &open(&mut rcv, &block)).unwrap() {
            let answers = respond(&t);
            seen.lock().unwrap().push(t);
            let ts: Vec<Vec<u8>> = answers
                .iter()
                .map(|(c, e, r)| {
                    encode_transmission(version, &[], c, e, &r.encode(version).unwrap()).unwrap()
                })
                .collect();
            for p in pack(&ts, BLOCK_SIZE - 18).0 {
                conn.send(&seal(&mut snd, &p))?;
            }
        }
    }
    Ok(())
}

fn seal(chain: &mut Option<SbKey>, payload: &[u8]) -> Vec<u8> {
    let Some(k) = chain else {
        return pad(payload, BLOCK_SIZE).unwrap();
    };
    let (key, nonce, next) = sbc_hkdf(k).unwrap();
    *k = next;
    sb_encrypt(&key, &nonce, payload, BLOCK_SIZE - 16).unwrap()
}

fn open(chain: &mut Option<SbKey>, block: &[u8]) -> Vec<u8> {
    let Some(k) = chain else {
        return unpad(block).unwrap().to_vec();
    };
    let (key, nonce, next) = sbc_hkdf(k).unwrap();
    *k = next;
    sb_decrypt(&key, &nonce, block).unwrap()
}
