//! A command out, and the bytes the session has for the socket (Client.hs `sendXFTPCommand`).
use crate::crypto::PrivateKey;
use crate::h2::H2Client;

use super::super::call::{encode_call, post};
use super::super::commands::Command;
use super::super::error::XftpError;
use super::super::version::BLOCK_SIZE;
use super::machine::{Phase, XftpSession};

impl XftpSession {
    /// Sends `cmd` signed with `key` and, after its block, `file`; the answer may carry `max_file`
    /// bytes after its block. One command at a time, once ready.
    pub fn send(
        &mut self,
        key: Option<&PrivateKey>,
        entity: &[u8],
        cmd: &Command,
        file: &[u8],
        max_file: usize,
        now: u64,
    ) -> Result<(), XftpError> {
        let (Phase::Ready(session), Some(h2), false) = (&self.phase, &mut self.h2, self.busy)
        else {
            return Err(XftpError::Unexpected(
                "a command while the session is not idle",
            ));
        };
        let body = encode_call(session, key, entity, cmd, file)?;
        if let Err(e) = h2.start(&post(&self.authority, &body), BLOCK_SIZE + max_file) {
            self.phase = Phase::Down; // GOAWAY came, or the connection failed
            return Err(e.into());
        }
        self.busy = true;
        let budget = self.cfg.command_budget_ms(file.len() + max_file);
        self.deadline = Some(now.saturating_add(budget));
        Ok(())
    }

    /// The next bytes for the socket: TLS records first, then HTTP/2 frames sealed in order.
    pub fn take_outgoing(&mut self) -> Option<Vec<u8>> {
        if let Some(bytes) = self.out.pop_front() {
            return Some(bytes);
        }
        let frames = self.h2.as_mut().and_then(H2Client::take_outgoing)?;
        match self.tls.write_app(&frames) {
            Ok(records) => Some(records),
            Err(_) => {
                // Only after a failed record, which `feed` has already reported.
                self.h2.as_mut().map(H2Client::abandon);
                None
            }
        }
    }
}
