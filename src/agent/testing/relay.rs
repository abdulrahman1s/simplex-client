//! The in-memory server as a proxy and as a destination relay (Server.hs `processProxiedCmd`, `processForwardedCommand`).
use std::sync::Arc;

use super::connector::write;
use super::queues::Push;
use super::server::FakeSmp;
use crate::crypto::{PublicKey, random_vec};
use crate::encoding::{Decode, Decoder, Encode, Version, VersionRange};
use crate::smp::proxy::destination::open_forwarded;
use crate::smp::proxy::{BrokerError, FwdTransmission, TransportError};
use crate::smp::testing::signed_key;
use crate::smp::{ErrorType, Incoming, ProxyError, Response, SmpServer};

fn split(b: &[u8]) -> (&[u8], &[u8]) {
    let at = b.iter().position(|x| *x == b' ').unwrap_or(b.len());
    (&b[..at], b.get(at + 1..).unwrap_or_default())
}

fn broker(e: BrokerError) -> Response {
    Response::Err(ErrorType::Proxy(ProxyError::Broker(e)))
}

impl FakeSmp {
    /// PRXY and PFWD; `None` for any other command.
    pub fn proxy_command(&self, t: &Incoming) -> Option<Response> {
        let (tag, rest) = split(&t.command);
        match tag {
            b"PRXY" => Some(self.prxy(rest)),
            b"PFWD" => Some(self.pfwd(t, rest)),
            _ => None,
        }
    }

    fn prxy(&self, rest: &[u8]) -> Response {
        self.lock().log.push("PRXY");
        let server = SmpServer::decode(&mut Decoder::new(rest)).unwrap();
        let dest = self
            .net
            .lock()
            .unwrap()
            .iter()
            .find(|s| s.address() == server)
            .cloned();
        let Some(dest) = dest else {
            return broker(BrokerError::Network);
        };
        let Some(pkey) = dest.open_relay_session() else {
            return broker(BrokerError::Transport(TransportError::Version));
        };
        if let Response::Pkey { session_id, .. } = &pkey {
            self.lock().relayed.insert(session_id.clone(), dest);
        }
        pkey
    }

    /// A session for a proxy, as its connection to this relay would be.
    fn open_relay_session(&self) -> Option<Response> {
        let mut i = self.lock();
        if i.no_forwarding {
            return None;
        }
        let session_id = random_vec(32).unwrap();
        i.proxy_sessions.insert(session_id.clone());
        Some(Response::Pkey {
            session_id,
            range: VersionRange::new(Version(15), Version(self.version)).unwrap(),
            chain: self.identity.chain.clone(),
            signed_key: signed_key(&i.server_key.public, &self.identity.leaf),
        })
    }

    fn pfwd(&self, t: &Incoming, rest: &[u8]) -> Response {
        let dest = {
            let mut i = self.lock();
            i.log.push("PFWD");
            i.relayed.get(&t.entity_id).cloned()
        };
        let Some(dest) = dest else {
            return Response::Err(ErrorType::Proxy(ProxyError::NoSession));
        };
        let mut d = Decoder::new(rest);
        let fwd = FwdTransmission {
            corr_id: t.corr_id.clone(),
            version: u16::decode(&mut d).unwrap(),
            key: PublicKey::decode(&mut d).unwrap(),
            body: d.take_all().to_vec(),
        };
        // What RFWD carries, read back as the relay reads it.
        let mut wire = Vec::new();
        fwd.encode(&mut wire).unwrap();
        dest.forwarded(
            &t.entity_id,
            &FwdTransmission::decode(&mut Decoder::new(&wire)).unwrap(),
        )
    }

    /// The destination's side: the command acts here as a sender's would, on no client session.
    fn forwarded(self: Arc<Self>, session: &[u8], fwd: &FwdTransmission) -> Response {
        let mut pushes: Vec<Push> = Vec::new();
        let (f, r, txs) = {
            let mut i = self.lock();
            if !i.proxy_sessions.contains(session) {
                return Response::Err(ErrorType::Proxy(ProxyError::NoSession));
            }
            let f = match open_forwarded(&i.server_key.private, fwd) {
                Ok(f) => f,
                Err(e) => {
                    return Response::Err(ErrorType::Proxy(ProxyError::Protocol(Box::new(e))));
                }
            };
            let name = split(&f.t.command).0.to_vec();
            i.forwarded.push(String::from_utf8(name).unwrap());
            let r = i.handle_in(f.version, 0, session, &f.t, &mut pushes);
            let txs: Vec<_> = pushes
                .into_iter()
                .filter_map(|(s, e, r)| Some((i.sessions.get(&s)?.tx.clone(), e, r)))
                .collect();
            (f, r, txs)
        };
        for (tx, entity, push) in txs {
            write(&tx, self.version, &[], &entity, &push);
        }
        let (nonce, body) = f.seal_answer(&r);
        Response::Pres { nonce, body }
    }
}
