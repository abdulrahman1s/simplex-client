//! The receiving half of a TLS connection: its own key, sequence number and socket handle.
use std::io::{self, Read};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use super::app_rx::{AppReceiver, Received};
use super::error::TlsError;
use super::info::TlsInfo;
use super::record::Inbound;
use super::shared::Shared;
use crate::net::Socket;

/// Reads application data. `Ok(0)` means the server sent close_notify. A read timeout leaves
/// the connection usable: call `read` again.
pub struct TlsReader<S: Socket> {
    sock: S,
    inbound: Inbound,
    rx: AppReceiver,
    app: Vec<u8>,
    app_pos: usize,
    closed: bool,
    shared: Arc<Shared>,
}

impl<S: Socket> TlsReader<S> {
    pub(super) fn new(
        sock: S,
        inbound: Inbound,
        mut rx: AppReceiver,
        handshake_buf: &[u8],
        shared: Arc<Shared>,
    ) -> Result<Self, TlsError> {
        if rx.handshake(handshake_buf)? {
            shared.key_update_requested.store(true, Ordering::Release);
        }
        Ok(Self {
            sock,
            inbound,
            rx,
            app: Vec::new(),
            app_pos: 0,
            closed: false,
            shared,
        })
    }

    #[must_use]
    pub fn info(&self) -> &TlsInfo {
        &self.shared.info
    }

    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.sock.set_read_timeout(timeout)
    }

    fn pump(&mut self) -> Result<(), TlsError> {
        let record = self.inbound.next_record(&mut self.sock)?;
        match self.rx.open(&record)? {
            Received::Data(content) => {
                self.app = content;
                self.app_pos = 0;
            }
            Received::Closed => self.closed = true,
            Received::Handshake { update_requested } => {
                if update_requested {
                    self.shared
                        .key_update_requested
                        .store(true, Ordering::Release);
                }
            }
        }
        Ok(())
    }
}

impl<S: Socket> Read for TlsReader<S> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        loop {
            let available = &self.app[self.app_pos..];
            if !available.is_empty() {
                let n = available.len().min(buf.len());
                buf[..n].copy_from_slice(&available[..n]);
                self.app_pos += n;
                return Ok(n);
            }
            if self.closed {
                return Ok(0);
            }
            self.pump()?;
        }
    }
}
