//! The client side of the TLS 1.3 handshake as a state machine fed one record at a time (RFC 8446 section 4).
use super::super::error::TlsError;
use super::super::info::TlsInfo;
use super::super::key_schedule::{Secret, Transcript};
use super::super::record::{Protection, RawRecord};
use super::client_hello;
use super::messages::{Message, Messages};
use crate::crypto::KeyPair;

/// Everything the record layer needs after the handshake.
pub(in crate::tls) struct Established {
    /// The client's closing flight, to send before any application data.
    pub(in crate::tls) flight: Vec<u8>,
    pub(in crate::tls) rx: Protection,
    pub(in crate::tls) rx_secret: Secret,
    pub(in crate::tls) tx: Protection,
    pub(in crate::tls) tx_secret: Secret,
    pub(in crate::tls) info: TlsInfo,
}

pub(super) struct Keys {
    pub(super) master: Secret,
    pub(super) client_hs: Secret,
    pub(super) server_hs: Secret,
}

/// What the server flight has settled so far.
pub(super) struct Flight {
    pub(super) keys: Keys,
    pub(super) alpn: Option<String>,
    pub(super) request: Option<Vec<u8>>,
    pub(super) chain: Vec<Vec<u8>>,
}

pub(super) enum Step {
    ServerHello,
    EncryptedExtensions(Keys),
    Certificate(Flight),
    CertificateVerify(Flight),
    Finished(Flight),
    Done,
}

pub(in crate::tls) struct Handshake {
    pub(super) offered: Vec<String>,
    pub(super) transcript: Transcript,
    pub(super) key: KeyPair,
    pub(super) messages: Messages,
    step: Step,
}

impl Handshake {
    /// The machine and the ClientHello record to send, offering `alpn`.
    pub(in crate::tls) fn start(alpn: &[&str]) -> Result<(Self, Vec<u8>), TlsError> {
        let mut transcript = Transcript::new();
        let hello = client_hello::build(alpn, &mut transcript)?;
        let offered = alpn.iter().map(|p| (*p).to_owned()).collect();
        let machine = Self {
            offered,
            transcript,
            key: hello.key,
            messages: Messages::default(),
            step: Step::ServerHello,
        };
        Ok((machine, hello.record))
    }

    /// Takes one record; keys change between messages, so records are opened strictly in order.
    pub(in crate::tls) fn on_record(
        &mut self,
        record: RawRecord,
    ) -> Result<Option<Established>, TlsError> {
        self.messages.absorb(record)?;
        while let Some(m) = self.messages.take() {
            if let Some(done) = self.on_message(&m)? {
                return Ok(Some(done));
            }
        }
        Ok(None)
    }

    fn on_message(&mut self, m: &Message) -> Result<Option<Established>, TlsError> {
        self.step = match std::mem::replace(&mut self.step, Step::Done) {
            Step::ServerHello => Step::EncryptedExtensions(self.server_hello(m)?),
            Step::EncryptedExtensions(keys) => Step::Certificate(self.extensions(keys, m)?),
            Step::Certificate(f) => self.certificate(f, m)?,
            Step::CertificateVerify(f) => Step::Finished(self.certificate_verify(f, m)?),
            Step::Finished(f) => return self.finished(f, m).map(Some),
            Step::Done => return Err(TlsError::Protocol("handshake message after Finished")),
        };
        Ok(None)
    }
}
