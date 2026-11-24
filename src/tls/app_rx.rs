//! The receiving direction after the handshake: opens records, follows KeyUpdate, sees close_notify (RFC 8446 sections 4.6, 6.1).
use super::alert::Alert;
use super::error::TlsError;
use super::key_schedule::{Secret, next_traffic_secret};
use super::record::{ALERT, APPLICATION_DATA, HANDSHAKE, Protection, RawRecord};
use super::wire::{append_handshake, take_handshake_message};

const NEW_SESSION_TICKET: u8 = 4;
const KEY_UPDATE: u8 = 24;

/// What one record carried.
pub(super) enum Received {
    Data(Vec<u8>),
    Closed,
    /// Post-handshake messages, already acted on; `update_requested` asks us to rotate our sending key.
    Handshake {
        update_requested: bool,
    },
}

pub(super) struct AppReceiver {
    rx: Protection,
    secret: Secret,
    /// Bounded by `append_handshake`.
    handshake_buf: Vec<u8>,
}

impl AppReceiver {
    pub(super) fn new(rx: Protection, secret: Secret) -> Self {
        Self {
            rx,
            secret,
            handshake_buf: Vec::new(),
        }
    }

    pub(super) fn open(&mut self, record: &RawRecord) -> Result<Received, TlsError> {
        if record.header[0] != APPLICATION_DATA {
            return Err(TlsError::Protocol("unprotected record after the handshake"));
        }
        let (ty, content) = self.rx.open(&record.header, &record.body)?;
        match ty {
            APPLICATION_DATA => Ok(Received::Data(content)),
            ALERT => {
                let alert = Alert::parse(&content)?;
                if !alert.is_close_notify() {
                    return Err(TlsError::Alert(alert));
                }
                Ok(Received::Closed)
            }
            HANDSHAKE => Ok(Received::Handshake {
                update_requested: self.handshake(&content)?,
            }),
            _ => Err(TlsError::Protocol("record type")),
        }
    }

    /// Tickets are ignored; KeyUpdate rotates the receiving key and must end its record, since what
    /// follows was sealed under the next key (RFC 8446 section 5.1). True when the peer asked us
    /// to rotate ours.
    fn handshake(&mut self, bytes: &[u8]) -> Result<bool, TlsError> {
        append_handshake(&mut self.handshake_buf, bytes)?;
        while let Some((ty, raw)) = take_handshake_message(&mut self.handshake_buf) {
            match ty {
                NEW_SESSION_TICKET => {}
                KEY_UPDATE if self.handshake_buf.is_empty() => return self.key_update(&raw[4..]),
                KEY_UPDATE => return Err(TlsError::Protocol("KeyUpdate spans a key change")),
                _ => return Err(TlsError::Protocol("unexpected post-handshake message")),
            }
        }
        Ok(false)
    }

    fn key_update(&mut self, body: &[u8]) -> Result<bool, TlsError> {
        let requested = match body {
            [0] => false,
            [1] => true,
            _ => return Err(TlsError::Illegal("KeyUpdate request")),
        };
        self.secret = next_traffic_secret(&self.secret);
        self.rx = Protection::new(&self.secret);
        Ok(requested)
    }
}
