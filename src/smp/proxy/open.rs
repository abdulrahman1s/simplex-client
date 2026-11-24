//! The answer to PFWD, opened (Client.hs `proxySMPCommand`, its answer half).
use crate::crypto::cb_decrypt;

use super::client_error::ProxyClientError;
use super::nonce::reverse_nonce;
use super::seal::Opener;
use crate::smp::error::SmpError;
use crate::smp::flow::Answer;
use crate::smp::responses::Response;
use crate::smp::server_error::{CommandError, ErrorType};
use crate::smp::transmission::parse_payload;

/// The destination's answer, or the proxy's own failure.
pub type Proxied = Result<Response, ProxyClientError>;

impl Opener {
    /// As the reference sorts the ten outcomes of a forwarded command: what the destination said
    /// (its ERR as `Server`, so AUTH and QUOTA read as if sent directly) and failures of the link
    /// to the proxy are the outer `Err`; the proxy's own ERR or nonsense is the inner one.
    pub fn open(&self, answer: Answer) -> Result<Proxied, SmpError> {
        let (nonce, body) = match answer {
            Ok(Response::Pres { nonce, body }) => (nonce, body),
            Ok(other) => return Ok(Err(ProxyClientError::Unexpected(other.name().into()))),
            Err(SmpError::Server(e)) => return Ok(Err(ProxyClientError::Protocol(e))),
            Err(SmpError::Unexpected(s)) => return Ok(Err(ProxyClientError::Unexpected(s.into()))),
            Err(SmpError::Decode(_)) => {
                let syntax = ErrorType::Cmd(CommandError::Syntax);
                return Ok(Err(ProxyClientError::Response(syntax)));
            }
            Err(e) => return Err(e),
        };
        let nonce = nonce.unwrap_or_else(|| reverse_nonce(&self.nonce));
        let plain = cb_decrypt(&self.secret, &nonce, &body)?;
        let mut ts = parse_payload(self.version, &plain)?;
        let (Some(t), None) = (ts.pop(), ts.pop()) else {
            return Err(SmpError::Unexpected(
                "forwarded answer is not one transmission",
            ));
        };
        match Response::decode(self.version, &t.command)? {
            Response::Err(e) => Err(SmpError::Server(e)),
            r => Ok(Ok(r)),
        }
    }
}
