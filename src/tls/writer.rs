//! The sending half of a TLS connection: its own key, sequence number and socket handle.
use std::io::{self, Write};
use std::net::Shutdown;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use super::app_tx::AppSender;
use super::error::TlsError;
use super::info::TlsInfo;
use super::shared::Shared;
use crate::net::Socket;

pub struct TlsWriter<S: Socket> {
    sock: S,
    tx: AppSender,
    shared: Arc<Shared>,
}

impl<S: Socket> TlsWriter<S> {
    pub(super) fn new(sock: S, tx: AppSender, shared: Arc<Shared>) -> Self {
        Self { sock, tx, shared }
    }

    #[must_use]
    pub fn info(&self) -> &TlsInfo {
        &self.shared.info
    }

    /// Rotates our sending key and tells the server to do the same for what it sends.
    pub fn request_key_update(&mut self) -> Result<(), TlsError> {
        let record = self.tx.key_update(true)?;
        self.sock.write_all(&record)?;
        Ok(())
    }

    /// Sends close_notify and shuts down the sending direction.
    pub fn close(&mut self) -> Result<(), TlsError> {
        let record = self.tx.close_notify()?;
        self.sock.write_all(&record)?;
        self.sock.shutdown(Shutdown::Write)?;
        Ok(())
    }

    fn send(&mut self, data: &[u8]) -> Result<(), TlsError> {
        let mut out = Vec::new();
        if self
            .shared
            .key_update_requested
            .swap(false, Ordering::AcqRel)
        {
            out.extend(self.tx.key_update(false)?);
        }
        out.extend(self.tx.seal_data(data)?);
        self.sock.write_all(&out)?;
        Ok(())
    }
}

impl<S: Socket> Write for TlsWriter<S> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        self.send(buf)?;
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.sock.flush()
    }
}
