//! Bytes from the network: TLS, then HTTP/2, then the handshake's steps or the command's answer.
use crate::h2::H2Client;

use super::super::call::{decode_answer, post};
use super::super::error::{HandshakeError, XftpError, tls_error};
use super::super::handshake::{
    Negotiated, TlsFacts, accept_ack, accept_server_hello, handshake_body, verify_identity,
};
use super::super::handshake_msgs::encode_client_handshake;
use super::super::version::{ALPN, BLOCK_SIZE};
use super::machine::{Phase, SessionEvent, XftpSession, ms};

impl XftpSession {
    /// Takes network bytes, any amount, and returns what they completed.
    pub fn feed(&mut self, bytes: &[u8], now: u64) -> Vec<SessionEvent> {
        let mut events = Vec::new();
        if let Err(e) = self.receive(bytes, now, &mut events) {
            self.fail(e, &mut events);
        }
        events
    }

    fn receive(
        &mut self,
        bytes: &[u8],
        now: u64,
        events: &mut Vec<SessionEvent>,
    ) -> Result<(), XftpError> {
        if self.is_down() {
            return Ok(());
        }
        let fed = self.tls.feed(bytes);
        while let Some(record) = self.tls.take_outgoing() {
            self.out.push_back(record);
        }
        fed.map_err(tls_error)?;
        if matches!(self.phase, Phase::Tls) && self.tls.is_ready() {
            self.tls_ready(now, events)?;
        }
        let app = self.tls.read_app();
        if let Some(h2) = self.h2.as_mut().filter(|_| !app.is_empty()) {
            h2.feed(&app)?;
        }
        if let Some(resp) = self.h2.as_mut().and_then(H2Client::take_response) {
            self.on_response(resp, now, events)?;
        }
        if self.tls.is_peer_closed() {
            return Err(XftpError::H2(crate::h2::H2Error::Closed));
        }
        Ok(())
    }

    /// TLS is up: the identity is checked and the handshake starts. A server without ALPN
    /// `xftp/1` is taken at version 1 with TLS's channel binding, as Haskell's client does.
    fn tls_ready(&mut self, now: u64, events: &mut Vec<SessionEvent>) -> Result<(), XftpError> {
        let mut h2 = H2Client::open(self.h2_opts);
        let alpn = self.tls.info().and_then(|i| i.alpn()) == Some(ALPN);
        let tls_unique = self
            .tls
            .tls_unique()
            .map(|u| u.to_vec())
            .unwrap_or_default();
        let facts = TlsFacts {
            tls_unique: &tls_unique,
            peer_chain: self.tls.peer_cert_chain(),
        };
        verify_identity(&facts, &self.key_hash, (self.cfg.wall_clock)())?;
        if alpn {
            h2.start(&post(&self.authority, &[]), BLOCK_SIZE)?;
            self.phase = Phase::Hello;
            self.deadline = Some(now.saturating_add(ms(self.cfg.connect_timeout)));
        } else {
            self.ready(Negotiated {
                version: 1,
                session_id: tls_unique,
            });
            events.push(SessionEvent::Ready);
        }
        self.h2 = Some(h2);
        Ok(())
    }

    fn on_response(
        &mut self,
        resp: crate::h2::Response,
        now: u64,
        events: &mut Vec<SessionEvent>,
    ) -> Result<(), XftpError> {
        match std::mem::replace(&mut self.phase, Phase::Down) {
            Phase::Hello => {
                let tls_unique = self
                    .tls
                    .tls_unique()
                    .map(|u| u.to_vec())
                    .unwrap_or_default();
                let facts = TlsFacts {
                    tls_unique: &tls_unique,
                    peer_chain: self.tls.peer_cert_chain(),
                };
                let n = accept_server_hello(&handshake_body(resp)?, &facts)?;
                let ours = encode_client_handshake(n.version, &self.key_hash)?;
                let h2 = self.h2.as_mut().ok_or(HandshakeError::Parse)?;
                h2.start(&post(&self.authority, &ours), BLOCK_SIZE)?;
                self.phase = Phase::Ack(n);
                self.deadline = Some(now.saturating_add(ms(self.cfg.connect_timeout)));
            }
            Phase::Ack(n) => {
                accept_ack(&handshake_body(resp)?)?;
                self.ready(n);
                events.push(SessionEvent::Ready);
            }
            Phase::Ready(n) => {
                let answer = decode_answer(&n, resp);
                self.ready(n);
                events.push(SessionEvent::Answer(answer));
            }
            Phase::Tls | Phase::Down => return Err(XftpError::Unexpected("an HTTP/2 response")),
        }
        Ok(())
    }

    fn ready(&mut self, n: Negotiated) {
        self.phase = Phase::Ready(n);
        self.busy = false;
        self.deadline = None;
    }
}
