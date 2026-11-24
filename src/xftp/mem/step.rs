//! What an in-memory transfer asks of its driver: one command on one server, then its answer.
use crate::crypto::PrivateKey;

use super::super::address::XftpServer;
use super::super::commands::Command;
use super::super::config::ATTEMPTS;
use super::super::error::XftpError;
use super::super::responses::Response;

/// A command to run on `server`; its answer goes to the transfer's `on_answer`.
pub struct Call {
    pub server: XftpServer,
    pub key: Option<PrivateKey>,
    pub entity: Vec<u8>,
    pub cmd: Command,
    /// Bytes after the command's block: a chunk for `FPUT`.
    pub file: Vec<u8>,
    /// Bytes the answer may carry after its block: a chunk for `FGET`.
    pub max_file: usize,
}

pub enum Step<T> {
    Call(Box<Call>),
    Done(T),
}

/// The answer to a `Call`, or why there is none.
pub type Answer = Result<(Response, Vec<u8>), XftpError>;

/// Tries of the command in hand. A driver drops its connection to the server after a
/// temporary failure, so each try is on a fresh one, as `Pool::run` does.
#[derive(Default)]
pub(super) struct Tries(usize);

impl Tries {
    /// Whether `e` leaves another try of the same command on the same server.
    pub(super) fn again(&mut self, e: &XftpError) -> bool {
        if !e.is_temporary() {
            return false;
        }
        self.0 += 1;
        self.0 < ATTEMPTS
    }

    pub(super) fn reset(&mut self) {
        self.0 = 0;
    }
}
