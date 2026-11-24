//! One handler per server handshake message, in the order RFC 8446 section 4.4.1 fixes.
use super::super::error::TlsError;
use super::super::info::TlsInfo;
use super::super::key_schedule::{certificate_verify_input, derive_secret, handshake_secrets};
use super::super::record::Protection;
use super::super::verify::check_certificate_verify;
use super::finished::{check_server_finished, client_flight};
use super::machine::{Established, Flight, Handshake, Keys, Step};
use super::messages::Message;
use super::server_flight::{
    CERTIFICATE, CERTIFICATE_REQUEST, CERTIFICATE_VERIFY, ENCRYPTED_EXTENSIONS, FINISHED,
    SERVER_HELLO, parse_certificate, parse_certificate_request, parse_certificate_verify,
    parse_encrypted_extensions, parse_server_hello,
};
use crate::crypto::{Alg, PublicKey, dh};

fn expect(m: &Message, ty: u8, what: &'static str) -> Result<(), TlsError> {
    if m.ty == ty {
        Ok(())
    } else {
        Err(TlsError::Protocol(what))
    }
}

impl Handshake {
    pub(super) fn server_hello(&mut self, m: &Message) -> Result<Keys, TlsError> {
        expect(m, SERVER_HELLO, "expected ServerHello")?;
        let peer = PublicKey::from_raw(Alg::X25519, &parse_server_hello(m.body())?)?;
        let shared = dh(&peer, &self.key.private)?;
        if shared.as_bytes().iter().all(|b| *b == 0) {
            return Err(TlsError::Illegal("all-zero shared secret"));
        }
        self.transcript.add(&m.raw);
        let secrets = handshake_secrets(shared.as_bytes());
        let hello_hash = self.transcript.hash();
        let server_hs = derive_secret(&secrets.handshake, "s hs traffic", &hello_hash);
        self.messages.rx = Some(Protection::new(&server_hs));
        Ok(Keys {
            master: secrets.master,
            client_hs: derive_secret(&secrets.handshake, "c hs traffic", &hello_hash),
            server_hs,
        })
    }

    pub(super) fn extensions(&mut self, keys: Keys, m: &Message) -> Result<Flight, TlsError> {
        expect(m, ENCRYPTED_EXTENSIONS, "expected EncryptedExtensions")?;
        let offered: Vec<&str> = self.offered.iter().map(String::as_str).collect();
        let alpn = parse_encrypted_extensions(m.body(), &offered)?;
        self.transcript.add(&m.raw);
        Ok(Flight {
            keys,
            alpn,
            request: None,
            chain: Vec::new(),
        })
    }

    /// Certificate, after at most one CertificateRequest.
    pub(super) fn certificate(&mut self, mut f: Flight, m: &Message) -> Result<Step, TlsError> {
        if m.ty == CERTIFICATE_REQUEST && f.request.is_none() {
            f.request = Some(parse_certificate_request(m.body())?);
            self.transcript.add(&m.raw);
            return Ok(Step::Certificate(f));
        }
        expect(m, CERTIFICATE, "expected Certificate")?;
        f.chain = parse_certificate(m.body())?;
        self.transcript.add(&m.raw);
        Ok(Step::CertificateVerify(f))
    }

    pub(super) fn certificate_verify(
        &mut self,
        f: Flight,
        m: &Message,
    ) -> Result<Flight, TlsError> {
        expect(m, CERTIFICATE_VERIFY, "expected CertificateVerify")?;
        let (scheme, signature) = parse_certificate_verify(m.body())?;
        let signed = certificate_verify_input(&self.transcript.hash(), true);
        let leaf = f.chain.first().ok_or(TlsError::EmptyChain)?;
        check_certificate_verify(leaf, scheme, signature, &signed)?;
        self.transcript.add(&m.raw);
        Ok(f)
    }

    pub(super) fn finished(&mut self, f: Flight, m: &Message) -> Result<Established, TlsError> {
        expect(m, FINISHED, "expected Finished")?;
        check_server_finished(&f.keys.server_hs, &self.transcript.hash(), m.body())?;
        self.transcript.add(&m.raw);
        let server_done = self.transcript.hash();
        let client_ap = derive_secret(&f.keys.master, "c ap traffic", &server_done);
        let server_ap = derive_secret(&f.keys.master, "s ap traffic", &server_done);
        let mut tx_hs = Protection::new(&f.keys.client_hs);
        let (flight, tls_unique) = client_flight(
            f.request.as_deref(),
            &mut self.transcript,
            &f.keys.client_hs,
            &mut tx_hs,
        )?;
        Ok(Established {
            flight,
            pending: std::mem::take(&mut self.messages.pending),
            rx: Protection::new(&server_ap),
            rx_secret: server_ap,
            tx: Protection::new(&client_ap),
            tx_secret: client_ap,
            info: TlsInfo {
                alpn: f.alpn,
                peer_cert_chain: f.chain,
                tls_unique,
                certificate_requested: f.request.is_some(),
            },
        })
    }
}
