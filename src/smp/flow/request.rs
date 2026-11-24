//! A command for a connection or a session, and the answer it gets (Client.hs `ProtocolClient` requests).
use std::time::Duration;

use crate::crypto::PrivateKey;
use crate::smp::commands::Command;
use crate::smp::error::SmpError;
use crate::smp::responses::Response;

/// The answer to one command: the response, or the server's ERR, a timeout, or the connection going down.
pub type Answer = Result<Response, SmpError>;

/// One command to send. Authorisation happens when it is sent, against the session it actually goes out on.
#[derive(Debug, Clone)]
pub struct Request {
    pub command: Command,
    /// Queue id the command acts on; empty for NEW and PING.
    pub entity_id: Vec<u8>,
    /// Signs (Ed25519/Ed448) or authenticates (X25519) the command; `None` for PING, LGET and an unsecured SEND.
    pub auth_key: Option<PrivateKey>,
    /// A 24-byte correlation id; random when `None`. Short-link NEW derives the sender id from it.
    pub corr_id: Option<Vec<u8>>,
    /// How long the answer may take; `ConnConfig::command_timeout` when `None`. A proxy needs
    /// longer, as it reaches another relay first (`connectSMPProxiedRelay`, `proxySMPCommand`).
    pub timeout: Option<Duration>,
}

impl Request {
    #[must_use]
    pub fn new(command: Command, entity_id: Vec<u8>, auth_key: Option<PrivateKey>) -> Self {
        Self {
            command,
            entity_id,
            auth_key,
            corr_id: None,
            timeout: None,
        }
    }

    #[must_use]
    pub const fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}
