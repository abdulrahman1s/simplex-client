//! State both halves of a split connection see: the handshake facts, a KeyUpdate request, a failure.
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};

use super::error::TlsError;
use super::info::TlsInfo;

pub(super) struct Shared {
    pub(super) info: TlsInfo,
    /// The peer sent KeyUpdate(update_requested); the writer answers before its next record.
    pub(super) key_update_requested: AtomicBool,
    /// A record failed to open or broke the protocol; every later read and write fails (RFC 8446 section 6).
    pub(super) failed: AtomicBool,
}

impl Shared {
    pub(super) fn new(info: TlsInfo) -> Self {
        Self {
            info,
            key_update_requested: AtomicBool::new(false),
            failed: AtomicBool::new(false),
        }
    }

    /// Marks the connection dead unless `e` only says the socket had nothing yet.
    pub(super) fn note(&self, e: &TlsError) {
        let transient = matches!(e, TlsError::Io(io) if matches!(
            io.kind(),
            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut | io::ErrorKind::Interrupted
        ));
        if !transient {
            self.failed.store(true, Ordering::Release);
        }
    }

    pub(super) fn check(&self) -> Result<(), TlsError> {
        if self.failed.load(Ordering::Acquire) {
            Err(TlsError::Protocol("the connection already failed"))
        } else {
            Ok(())
        }
    }
}
