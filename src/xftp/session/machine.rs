//! One XFTP session driven by pushed bytes and passed-in time (Client.hs `XFTPClient`).
use std::collections::VecDeque;

use crate::h2::{self, H2Client, Progress};
use crate::tls::{Pin, TlsClient};

use super::super::address::{XftpServer, authority};
use super::super::config::ClientConfig;
use super::super::error::{XftpError, tls_error};
use super::super::handshake::Negotiated;
use super::super::responses::Response;
use super::super::version::ALPN;

/// What a session tells its owner.
#[derive(Debug)]
pub enum SessionEvent {
    /// The handshake finished; commands may be sent.
    Ready,
    /// The answer to the command in flight, and the bytes after its block. A failure of the
    /// connection itself ends the session too (`is_down`).
    Answer(Result<(Response, Vec<u8>), XftpError>),
    /// The session ended with no command in flight.
    Closed(XftpError),
}

pub(super) enum Phase {
    Tls,
    /// The first handshake POST is in flight.
    Hello,
    /// Our handshake block is in flight.
    Ack(Negotiated),
    Ready(Negotiated),
    Down,
}

/// A session never reconnects: after it is down the owner starts a new one.
pub struct XftpSession {
    pub(super) tls: TlsClient,
    /// Made once TLS is up, so its preface goes out encrypted.
    pub(super) h2: Option<H2Client>,
    pub(super) h2_opts: h2::Options,
    pub(super) phase: Phase,
    pub(super) key_hash: Vec<u8>,
    pub(super) authority: String,
    pub(super) cfg: ClientConfig,
    /// When the step under way (connecting, a handshake POST, a command) fails, if ever.
    pub(super) deadline: Option<u64>,
    /// A command is in flight.
    pub(super) busy: bool,
    /// Bytes for the socket in the order they were sealed; the owner drains it after every call.
    pub(super) out: VecDeque<Vec<u8>>,
}

pub(super) fn ms(d: std::time::Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

impl XftpSession {
    /// The session and the TLS ClientHello to send once the byte pipe to `host` is open.
    /// `h2_opts.send_ahead` bounds the upload bytes the server has not acknowledged.
    pub fn start(
        server: &XftpServer,
        host: &str,
        cfg: ClientConfig,
        h2_opts: h2::Options,
        now: u64,
    ) -> Result<(Self, Vec<u8>), XftpError> {
        let pin = Pin {
            key_hash: server.key_hash.clone(),
            now: (cfg.wall_clock)(),
        };
        let (tls, hello) = TlsClient::start_pinned(&[ALPN], pin).map_err(tls_error)?;
        let session = Self {
            tls,
            h2: None,
            h2_opts,
            phase: Phase::Tls,
            key_hash: server.key_hash.clone(),
            authority: authority(host, server.port_or_default()),
            cfg,
            deadline: Some(now.saturating_add(ms(cfg.connect_timeout))),
            busy: false,
            out: VecDeque::new(),
        };
        Ok((session, hello))
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        matches!(self.phase, Phase::Ready(_))
    }

    /// Ready, with no command in flight.
    #[must_use]
    pub fn is_idle(&self) -> bool {
        self.is_ready() && !self.busy
    }

    #[must_use]
    pub fn is_down(&self) -> bool {
        matches!(self.phase, Phase::Down)
    }

    /// The negotiated version, once ready.
    #[must_use]
    pub fn version(&self) -> Option<u16> {
        match &self.phase {
            Phase::Ready(n) => Some(n.version),
            Phase::Tls | Phase::Hello | Phase::Ack(_) | Phase::Down => None,
        }
    }

    /// Body bytes the command in flight has moved, its block included.
    #[must_use]
    pub fn progress(&self) -> Progress {
        self.h2.as_ref().map(H2Client::progress).unwrap_or_default()
    }

    #[must_use]
    pub fn next_deadline(&self) -> Option<u64> {
        self.deadline
    }
}
