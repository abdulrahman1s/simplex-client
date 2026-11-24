//! An XFTP connection: one command in flight over HTTP/2, each a padded block (Client.hs `sendXFTPCommand`).
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::crypto::PrivateKey;
use crate::h2::{Connection, Progress, Stop};
use crate::smp::Clock;

use super::call::{decode_answer, encode_call, post};
use super::commands::Command;
pub use super::config::ClientConfig;
use super::error::XftpError;
use super::handshake::Negotiated;
use super::responses::Response;
use super::version::BLOCK_SIZE;

/// What a running command needs from its caller.
pub struct Ctl<'a> {
    pub cancel: &'a AtomicBool,
    pub clock: &'a dyn Clock,
    /// Body bytes moved by the current command.
    pub on_progress: &'a mut dyn FnMut(Progress),
}

pub struct XftpClient<T: Read + Write> {
    pub(super) conn: Connection<T>,
    pub(super) authority: String,
    pub(super) session: Negotiated,
    pub(super) cfg: ClientConfig,
}

impl<T: Read + Write> XftpClient<T> {
    #[must_use]
    pub fn version(&self) -> u16 {
        self.session.version
    }

    #[must_use]
    pub fn is_usable(&self) -> bool {
        self.conn.is_usable()
    }

    /// Sends `cmd` and, after it, `file` bytes; returns the answer and the bytes after the response block.
    /// `max_file` bounds those. A server `ERR` is an `Err`.
    pub(super) fn call(
        &mut self,
        key: Option<&PrivateKey>,
        entity: &[u8],
        cmd: &Command,
        file: &[u8],
        max_file: usize,
        ctl: &mut Ctl<'_>,
    ) -> Result<(Response, Vec<u8>), XftpError> {
        let body = encode_call(&self.session, key, entity, cmd, file)?;
        let deadline = ctl.clock.now_ms() + self.cfg.command_budget_ms(file.len() + max_file);
        let (cancel, clock) = (ctl.cancel, ctl.clock);
        let on_progress = &mut *ctl.on_progress;
        let mut watch = |p: Progress| -> Result<(), Stop> {
            on_progress(p);
            if cancel.load(Ordering::Relaxed) {
                return Err(Stop::Cancelled);
            }
            if clock.now_ms() > deadline {
                Err(Stop::TimedOut)
            } else {
                Ok(())
            }
        };
        let req = post(&self.authority, &body);
        let resp = self.conn.request(&req, BLOCK_SIZE + max_file, &mut watch)?;
        decode_answer(&self.session, resp)
    }
}
