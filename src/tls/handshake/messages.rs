//! Handshake messages out of plain and protected records during the handshake (RFC 8446 section 5.1).
use super::super::alert::Alert;
use super::super::error::TlsError;
use super::super::record::{ALERT, APPLICATION_DATA, CCS, HANDSHAKE, Protection, RawRecord};
use super::super::wire::{append_handshake, take_handshake_message};

/// A handshake message with its 4-byte header, as the transcript hashes it.
pub(super) struct Message {
    pub(super) ty: u8,
    pub(super) raw: Vec<u8>,
}

impl Message {
    pub(super) fn body(&self) -> &[u8] {
        &self.raw[4..]
    }
}

#[derive(Default)]
pub(super) struct Messages {
    /// The server's handshake key, once ServerHello has given it.
    pub(super) rx: Option<Protection>,
    /// Handshake bytes taken out of records but not yet whole messages; bounded by `append_handshake`.
    pub(super) pending: Vec<u8>,
}

impl Messages {
    pub(super) fn take(&mut self) -> Option<Message> {
        take_handshake_message(&mut self.pending).map(|(ty, raw)| Message { ty, raw })
    }

    /// Opens one record and keeps its handshake bytes; an alert or any other content fails the handshake.
    pub(super) fn absorb(&mut self, record: RawRecord) -> Result<(), TlsError> {
        if record.header[0] == CCS {
            // Middlebox-compatibility ChangeCipherSpec (RFC 8446 appendix D.4).
            return if record.body == [1] {
                Ok(())
            } else {
                Err(TlsError::Illegal("ChangeCipherSpec"))
            };
        }
        let (ty, content) = match self.rx.as_mut() {
            None => (record.header[0], record.body),
            Some(rx) if record.header[0] == APPLICATION_DATA => {
                rx.open(&record.header, &record.body)?
            }
            Some(_) => return Err(TlsError::Protocol("unprotected record after keys")),
        };
        match ty {
            HANDSHAKE => append_handshake(&mut self.pending, &content),
            ALERT => Err(TlsError::Alert(Alert::parse(&content)?)),
            _ => Err(TlsError::Protocol("record type during the handshake")),
        }
    }
}
