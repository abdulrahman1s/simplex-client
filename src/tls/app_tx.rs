//! The sending direction after the handshake: application records, KeyUpdate and close_notify (RFC 8446 sections 4.6.3, 6.1).
use super::alert::Alert;
use super::error::TlsError;
use super::key_schedule::{Secret, next_traffic_secret};
use super::record::{ALERT, APPLICATION_DATA, HANDSHAKE, MAX_PLAINTEXT, Protection};
use super::wire::handshake_message;

const KEY_UPDATE: u8 = 24;

pub(super) struct AppSender {
    tx: Protection,
    secret: Secret,
}

impl AppSender {
    pub(super) fn new(tx: Protection, secret: Secret) -> Self {
        Self { tx, secret }
    }

    /// `data` as records of at most 2^14 bytes of plaintext each.
    pub(super) fn seal_data(&mut self, data: &[u8]) -> Result<Vec<u8>, TlsError> {
        let mut out = Vec::with_capacity(data.len() + 64);
        for chunk in data.chunks(MAX_PLAINTEXT) {
            out.extend(self.tx.seal(APPLICATION_DATA, chunk)?);
        }
        Ok(out)
    }

    /// The KeyUpdate record under the old key; later records use the next key.
    pub(super) fn key_update(&mut self, request_peer: bool) -> Result<Vec<u8>, TlsError> {
        let message = handshake_message(KEY_UPDATE, &[u8::from(request_peer)]);
        let record = self.tx.seal(HANDSHAKE, &message)?;
        self.secret = next_traffic_secret(&self.secret);
        self.tx = Protection::new(&self.secret);
        Ok(record)
    }

    pub(super) fn close_notify(&mut self) -> Result<Vec<u8>, TlsError> {
        self.tx.seal(ALERT, &Alert::CLOSE_NOTIFY.bytes())
    }
}
