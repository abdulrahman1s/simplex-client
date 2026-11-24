//! The mock server's application phase handed to a test: application data both ways under the traffic keys.
use std::io::Write;

use super::pipe::Pipe;
use crate::tls::error::TlsError;
use crate::tls::key_schedule::Secret;
use crate::tls::record::{ALERT, APPLICATION_DATA, Inbound, MAX_PLAINTEXT, Protection};

pub(crate) struct ServerConn {
    sock: Pipe,
    inbound: Inbound,
    tx: Protection,
    rx: Protection,
    data: Vec<u8>,
    /// The client Finished verify_data, which SMP uses as its session id.
    pub(crate) tls_unique: Vec<u8>,
}

impl ServerConn {
    pub(super) fn new(
        sock: Pipe,
        inbound: Inbound,
        server_ap: &Secret,
        client_ap: &Secret,
        tls_unique: &[u8],
    ) -> Self {
        Self {
            sock,
            inbound,
            tx: Protection::new(server_ap),
            rx: Protection::new(client_ap),
            data: Vec::new(),
            tls_unique: tls_unique.to_vec(),
        }
    }

    pub(crate) fn send(&mut self, data: &[u8]) -> Result<(), TlsError> {
        for chunk in data.chunks(MAX_PLAINTEXT) {
            let record = self.tx.seal(APPLICATION_DATA, chunk)?;
            self.sock.write_all(&record)?;
        }
        Ok(())
    }

    /// Exactly `n` bytes of application data; an error once the client closes.
    pub(crate) fn recv_exact(&mut self, n: usize) -> Result<Vec<u8>, TlsError> {
        while self.data.len() < n {
            let r = self.inbound.next_record(&mut self.sock)?;
            match self.rx.open(&r.header, &r.body)? {
                (APPLICATION_DATA, d) => self.data.extend(d),
                (ALERT, _) => return Err(TlsError::Protocol("the client closed")),
                _ => return Err(TlsError::Protocol("unexpected client record")),
            }
        }
        Ok(self.data.drain(..n).collect())
    }
}
