//! An XFTP server's state and answers, enough for whole transfers: FNEW, FADD, FPUT, FGET, FACK, FDEL, PING.
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::crypto::{Alg, CbNonce, KeyPair, PublicKey, SbKey, SbState, dh, random_array, unpad};
use crate::encoding::{Decode, Decoder, Encode, Large, NonEmpty};
use crate::xftp::encode_transmission;

#[derive(Default)]
pub struct Store {
    /// Chunk bytes by sender id.
    pub chunks: HashMap<Vec<u8>, Vec<u8>>,
    /// Recipient id to sender id.
    pub recipients: HashMap<Vec<u8>, Vec<u8>>,
    /// Every command word received, in order.
    pub seen: Vec<String>,
    /// Answer the next this many commands with `ERR SESSION`, a temporary failure.
    pub fail_next: usize,
}

pub type Shared = Arc<Mutex<Store>>;

fn id() -> Vec<u8> {
    random_array::<24>().unwrap().to_vec()
}

fn ids(n: usize) -> Vec<Vec<u8>> {
    (0..n).map(|_| id()).collect()
}

/// The entity, command and file bytes of a request body.
fn parse(body: &[u8]) -> (Vec<u8>, Vec<u8>, Vec<u8>) {
    let (block, file) = body.split_at(crate::xftp::BLOCK_SIZE);
    let raw = unpad(block).unwrap();
    let mut d = Decoder::new(raw);
    assert_eq!(d.byte().unwrap(), 1);
    let payload = Large::decode(&mut d).unwrap().0;
    let mut t = Decoder::new(&payload);
    for _ in 0..3 {
        Vec::<u8>::decode(&mut t).unwrap(); // authenticator, session id, correlation id
    }
    let entity = Vec::<u8>::decode(&mut t).unwrap();
    (entity, t.take_all().to_vec(), file.to_vec())
}

/// The response block and any bytes after it for one command.
pub fn answer(store: &Shared, session_id: &[u8], version: u16, body: &[u8]) -> Vec<u8> {
    let (entity, cmd, file) = parse(body);
    let (word, rest) = cmd.split_at(cmd.iter().position(|b| *b == b' ').unwrap_or(cmd.len()));
    let mut d = Decoder::new(rest.get(1..).unwrap_or_default());
    let mut s = store.lock().unwrap();
    s.seen.push(String::from_utf8_lossy(word).into_owned());
    let (resp, extra) = if s.fail_next > 0 {
        s.fail_next -= 1;
        (b"ERR SESSION".to_vec(), Vec::new())
    } else {
        command(&mut s, word, &mut d, &entity, file, version)
    };
    let block = encode_transmission(session_id, &entity, &resp, None).unwrap();
    [block, extra].concat()
}

fn command(
    s: &mut Store,
    word: &[u8],
    d: &mut Decoder<'_>,
    entity: &[u8],
    file: Vec<u8>,
    version: u16,
) -> (Vec<u8>, Vec<u8>) {
    let mut out = Vec::new();
    match word {
        b"FNEW" => {
            PublicKey::decode(d).unwrap();
            u32::decode(d).unwrap();
            Vec::<u8>::decode(d).unwrap();
            let keys = NonEmpty::<PublicKey>::decode(d).unwrap().into_vec();
            let sender = id();
            let rcv = ids(keys.len());
            for r in &rcv {
                s.recipients.insert(r.clone(), sender.clone());
            }
            s.chunks.insert(sender.clone(), Vec::new());
            out.extend(b"SIDS ");
            sender.encode(&mut out).unwrap();
            NonEmpty::new(rcv).unwrap().encode(&mut out).unwrap();
            if version >= 4 {
                out.push(b'0');
            }
        }
        b"FADD" => {
            let keys = NonEmpty::<PublicKey>::decode(d).unwrap().into_vec();
            let rcv = ids(keys.len());
            for r in &rcv {
                s.recipients.insert(r.clone(), entity.to_vec());
            }
            out.extend(b"RIDS ");
            NonEmpty::new(rcv).unwrap().encode(&mut out).unwrap();
        }
        b"FPUT" => {
            s.chunks.insert(entity.to_vec(), file);
            out.extend(b"OK");
        }
        b"FGET" => return fget(s, d, entity),
        b"FACK" | b"FDEL" => out.extend(b"OK"),
        b"PING" => out.extend(b"PONG"),
        _ => out.extend(b"ERR CMD UNKNOWN"),
    }
    (out, Vec::new())
}

/// The chunk under a key agreed with the recipient's, then its tag.
fn fget(s: &Store, d: &mut Decoder<'_>, entity: &[u8]) -> (Vec<u8>, Vec<u8>) {
    let theirs = PublicKey::decode(d).unwrap();
    let Some(chunk) = s.recipients.get(entity).and_then(|snd| s.chunks.get(snd)) else {
        return (b"ERR AUTH".to_vec(), Vec::new());
    };
    let ours = KeyPair::generate(Alg::X25519).unwrap();
    let nonce = CbNonce::random().unwrap();
    let secret = dh(&theirs, &ours.private).unwrap();
    let mut state = SbState::new(&SbKey::from_slice(secret.as_bytes()).unwrap(), &nonce);
    let mut sealed = chunk.clone();
    state.encrypt(&mut sealed);
    sealed.extend(state.tag());
    let mut out = b"FILE ".to_vec();
    ours.public.encode(&mut out).unwrap();
    out.extend(nonce.as_bytes());
    (out, sealed)
}
