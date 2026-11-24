//! A proxy and the destination relays behind it in one place: PRXY opens a session, PFWD hands the sealed command over as RFWD would, the destination answers.
use std::collections::HashMap;

use super::certs::{Identity, identity, signed_key};
use super::support::keypair;
use super::tls_server::address;
use crate::crypto::{Alg, CbNonce, KeyPair, PublicKey, Signature, cb_verify, random_vec};
use crate::encoding::{Decode, Decoder, Encode, Tail, Version, VersionRange};
use crate::smp::proxy::destination::open_forwarded;
use crate::smp::proxy::{BrokerError, FwdTransmission};
use crate::smp::{CommandError, ErrorType, Incoming, ProxyError, Response, SmpServer, authorized};

#[derive(Default)]
pub struct Queue {
    pub sender_key: Option<PublicKey>,
    pub msgs: Vec<Vec<u8>>,
}

pub struct Dest {
    pub address: SmpServer,
    pub identity: Identity,
    pub key: KeyPair,
    pub max_version: u16,
    /// By sender id.
    pub queues: HashMap<Vec<u8>, Queue>,
    /// The proxy session each command came in on; nothing reaches this relay any other way.
    pub forwarded: Vec<Vec<u8>>,
}

impl Dest {
    pub fn new(snd_id: &[u8]) -> Self {
        let identity = identity();
        Self {
            address: address(&identity.key_hash),
            identity,
            key: keypair(Alg::X25519),
            max_version: 23,
            queues: HashMap::from([(snd_id.to_vec(), Queue::default())]),
            forwarded: Vec::new(),
        }
    }

    fn authorised_by(&self, sid: &[u8], t: &Incoming, key: &PublicKey) -> bool {
        let msg = authorized(sid, &t.corr_id, &t.entity_id, &t.command).unwrap();
        match key.alg() {
            Alg::X25519 => CbNonce::from_slice(&t.corr_id)
                .is_ok_and(|n| cb_verify(key, &self.key.private, &n, &t.auth, &msg)),
            alg @ (Alg::Ed25519 | Alg::Ed448) => {
                Signature::from_raw(alg, &t.auth).is_ok_and(|s| key.verify(&s, &msg))
            }
            Alg::X448 => false,
        }
    }

    /// SKEY and SEND, authorised under the proxy's session, as Server.hs `verifyTransmission` would.
    fn handle(&mut self, sid: &[u8], t: &Incoming) -> Response {
        self.forwarded.push(sid.to_vec());
        let (tag, rest) = split(&t.command);
        let mut d = Decoder::new(rest);
        let sender = self
            .queues
            .get(&t.entity_id)
            .and_then(|q| q.sender_key.clone());
        match tag {
            b"SKEY" => {
                let k = PublicKey::decode(&mut d).unwrap();
                if !self.authorised_by(sid, t, &k) || sender.is_some_and(|s| s != k) {
                    return Response::Err(ErrorType::Auth);
                }
                self.queues.get_mut(&t.entity_id).unwrap().sender_key = Some(k);
                Response::Ok
            }
            b"SEND" => {
                let ok = match &sender {
                    Some(k) => self.authorised_by(sid, t, k),
                    None => t.auth.is_empty(),
                };
                let Some(q) = self.queues.get_mut(&t.entity_id).filter(|_| ok) else {
                    return Response::Err(ErrorType::Auth);
                };
                let _flags = d.byte().unwrap();
                let _space = d.byte().unwrap();
                q.msgs.push(Tail::decode(&mut d).unwrap().0);
                Response::Ok
            }
            _ => Response::Err(ErrorType::Cmd(CommandError::Prohibited)),
        }
    }
}

fn split(b: &[u8]) -> (&[u8], &[u8]) {
    let at = b.iter().position(|x| *x == b' ').unwrap_or(b.len());
    (&b[..at], b.get(at + 1..).unwrap_or_default())
}

#[derive(Default)]
pub struct ProxyRelay {
    pub dests: Vec<Dest>,
    /// Session id to the destination's index.
    pub sessions: HashMap<Vec<u8>, usize>,
}

impl ProxyRelay {
    pub fn answer(&mut self, t: &Incoming) -> Response {
        let (tag, rest) = split(&t.command);
        match tag {
            b"PRXY" => self.session(rest),
            b"PFWD" => self.forward(t, rest),
            _ => Response::Err(ErrorType::Cmd(CommandError::Unknown)),
        }
    }

    fn session(&mut self, rest: &[u8]) -> Response {
        let server = SmpServer::decode(&mut Decoder::new(rest)).unwrap();
        let Some(i) = self.dests.iter().position(|d| d.address == server) else {
            return Response::Err(ErrorType::Proxy(ProxyError::Broker(BrokerError::Network)));
        };
        let d = &self.dests[i];
        let session_id = random_vec(32).unwrap();
        self.sessions.insert(session_id.clone(), i);
        Response::Pkey {
            session_id,
            range: VersionRange::new(Version(15), Version(d.max_version)).unwrap(),
            chain: d.identity.chain.clone(),
            signed_key: signed_key(&d.key.public, &d.identity.leaf),
        }
    }

    fn forward(&mut self, t: &Incoming, rest: &[u8]) -> Response {
        let Some(&i) = self.sessions.get(&t.entity_id) else {
            return Response::Err(ErrorType::Proxy(ProxyError::NoSession));
        };
        let mut d = Decoder::new(rest);
        let fwd = FwdTransmission {
            corr_id: t.corr_id.clone(),
            version: u16::decode(&mut d).unwrap(),
            key: PublicKey::decode(&mut d).unwrap(),
            body: d.take_all().to_vec(),
        };
        // What the proxy writes into RFWD, read back as the relay reads it.
        let mut wire = Vec::new();
        fwd.encode(&mut wire).unwrap();
        let fwd = FwdTransmission::decode(&mut Decoder::new(&wire)).unwrap();
        let dest = &mut self.dests[i];
        match open_forwarded(&dest.key.private, &fwd) {
            Ok(f) => {
                let r = dest.handle(&t.entity_id, &f.t);
                let (nonce, body) = f.seal_answer(&r);
                Response::Pres { nonce, body }
            }
            Err(e) => Response::Err(ErrorType::Proxy(ProxyError::Protocol(Box::new(e)))),
        }
    }
}
