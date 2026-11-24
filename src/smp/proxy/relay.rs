//! A proxy's session with a destination relay, checked against the relay's address (Client.hs `connectSMPProxiedRelay`, `ProxiedRelay`).
use std::time::Duration;

use crate::crypto::PublicKey;
use crate::encoding::{Version, VersionRange};
use crate::tls::verify_server_chain;

use super::broker_error::BrokerError;
use super::error::ProxyError;
use super::transport_error::{Handshake, TransportError};
use crate::smp::address::SmpServer;
use crate::smp::clock::system_wall_clock;
use crate::smp::commands::Command;
use crate::smp::error::SmpError;
use crate::smp::flow::{Answer, Request};
use crate::smp::handshake::negotiate;
use crate::smp::responses::Response;
use crate::smp::server_error::ErrorType;
use crate::smp::signed_key::verify_signed_key;
use crate::smp::version::{MAX_VERSION, MIN_VERSION};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProxiedRelay {
    /// The proxy's TLS session with the relay: the PFWD entity id, and what inner commands are authorised under.
    pub session_id: Vec<u8>,
    pub version: u16,
    /// Kept so that a session the proxy lost can be asked for again the same way.
    pub basic_auth: Option<String>,
    /// The relay's X25519 key that forwarded commands are sealed to.
    pub server_key: PublicKey,
}

/// PRXY for `dest` through a proxy that takes `basic_auth`. The proxy may have to connect to
/// `dest` first, so the answer may take a connect timeout on top of a command timeout.
#[must_use]
pub fn session_request(dest: &SmpServer, basic_auth: Option<String>, timeout: Duration) -> Request {
    let command = Command::Prxy {
        server: dest.clone(),
        basic_auth,
    };
    Request::new(command, Vec::new(), None).with_timeout(timeout)
}

fn transport(e: TransportError) -> SmpError {
    SmpError::Server(ErrorType::Proxy(ProxyError::Broker(
        BrokerError::Transport(e),
    )))
}

impl ProxiedRelay {
    /// The PKEY answering PRXY for `dest`. The relay must speak a version we do, its chain must be
    /// `dest`'s identity as the address pins it, and its key must be signed by that chain's leaf;
    /// failures read as the proxy's `PROXY BROKER TRANSPORT` errors, as in the reference.
    pub fn from_answer(
        dest: &SmpServer,
        basic_auth: Option<String>,
        answer: Answer,
    ) -> Result<Self, SmpError> {
        let Response::Pkey {
            session_id,
            range,
            chain,
            signed_key,
        } = answer?
        else {
            return Err(SmpError::Unexpected("answer to PRXY"));
        };
        let ours = VersionRange::new(Version(MIN_VERSION), Version(MAX_VERSION))
            .ok_or_else(|| transport(TransportError::Version))?;
        let version = negotiate(&range, &ours).map_err(|_| transport(TransportError::Version))?;
        let identity = || transport(TransportError::Handshake(Handshake::Identity));
        // The agent's signature carries no clock, so the system's wall clock dates the chain.
        let now = system_wall_clock();
        verify_server_chain(&chain, &dest.key_hash, now).map_err(|_| identity())?;
        let leaf = chain.first().ok_or_else(identity)?;
        let server_key = verify_signed_key(leaf, &signed_key).map_err(|_| identity())?;
        Ok(Self {
            session_id,
            version,
            basic_auth,
            server_key,
        })
    }
}
