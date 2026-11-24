//! A sans-I/O TLS 1.3 client (RFC 8446): network bytes are pushed in and bytes to send are taken out; the same handshake and records as `TlsStream`.
use std::collections::VecDeque;

use super::app_rx::{AppReceiver, Received};
use super::app_tx::AppSender;
use super::error::TlsError;
use super::handshake::{Established, Handshake};
use super::identity::Pin;
use super::info::TlsInfo;
use super::record::{Inbound, RawRecord};

struct Ready {
    rx: AppReceiver,
    tx: AppSender,
    info: TlsInfo,
}

enum Phase {
    Handshake(Box<Handshake>),
    Ready(Box<Ready>),
    /// A record failed; its key has already advanced, so nothing after it can be opened.
    Failed,
}

pub struct TlsClient {
    /// Holds at most one partial record once `feed` returns: 5 + 2^14 + 256 bytes.
    inbound: Inbound,
    phase: Phase,
    /// Records to send, oldest first: the client Finished flight and KeyUpdate answers. The owner
    /// drains it after each `feed`, and `write_app`/`close` drain it into what they return.
    outgoing: VecDeque<Vec<u8>>,
    /// Decrypted data not yet taken; bounded by what was fed since the owner last took it.
    app: Vec<u8>,
    peer_closed: bool,
}

impl TlsClient {
    /// The client and the ClientHello to send first, offering `alpn` (`smp/1`, `xftp/1`; empty
    /// offers none). Any certificate chain is taken: check it, or use `start_pinned`.
    pub fn start(alpn: &[&str]) -> Result<(Self, Vec<u8>), TlsError> {
        Self::open(alpn, None)
    }

    /// Like `start`, but a chain that is not `pin`'s identity fails the handshake with
    /// `TlsError::Identity` before our Finished is queued.
    pub fn start_pinned(alpn: &[&str], pin: Pin) -> Result<(Self, Vec<u8>), TlsError> {
        Self::open(alpn, Some(pin))
    }

    fn open(alpn: &[&str], pin: Option<Pin>) -> Result<(Self, Vec<u8>), TlsError> {
        let (handshake, hello) = Handshake::start(alpn, pin)?;
        let client = Self {
            inbound: Inbound::default(),
            phase: Phase::Handshake(Box::new(handshake)),
            outgoing: VecDeque::new(),
            app: Vec::new(),
            peer_closed: false,
        };
        Ok((client, hello))
    }

    /// Takes bytes from the network, any amount; complete records are processed at once.
    /// After an error the connection is dead and every later call fails.
    pub fn feed(&mut self, bytes: &[u8]) -> Result<(), TlsError> {
        if matches!(self.phase, Phase::Failed) {
            return Err(TlsError::Protocol("the connection already failed"));
        }
        self.inbound.push(bytes);
        let processed = self.process();
        if processed.is_err() {
            self.phase = Phase::Failed;
        }
        processed
    }

    fn process(&mut self) -> Result<(), TlsError> {
        while let Some(record) = self.inbound.take_record()? {
            self.on_record(record)?;
        }
        Ok(())
    }

    fn on_record(&mut self, record: RawRecord) -> Result<(), TlsError> {
        if self.peer_closed {
            return Err(TlsError::Protocol("record after close_notify"));
        }
        match &mut self.phase {
            Phase::Handshake(handshake) => {
                if let Some(est) = handshake.on_record(record)? {
                    self.phase = Phase::Ready(Box::new(self.established(est)));
                }
            }
            Phase::Ready(ready) => match ready.rx.open(&record)? {
                Received::Data(data) => self.app.extend(data),
                Received::Closed => self.peer_closed = true,
                Received::Handshake { update_requested } => {
                    if update_requested {
                        self.outgoing.push_back(ready.tx.key_update(false)?);
                    }
                }
            },
            Phase::Failed => return Err(TlsError::Protocol("the connection already failed")),
        }
        Ok(())
    }

    fn established(&mut self, est: Established) -> Ready {
        self.outgoing.push_back(est.flight);
        Ready {
            rx: AppReceiver::new(est.rx, est.rx_secret),
            tx: AppSender::new(est.tx, est.tx_secret),
            info: est.info,
        }
    }

    /// The next bytes to send, oldest first.
    pub fn take_outgoing(&mut self) -> Option<Vec<u8>> {
        self.outgoing.pop_front()
    }

    #[must_use]
    pub fn is_ready(&self) -> bool {
        matches!(self.phase, Phase::Ready(_))
    }

    /// The handshake's facts, once it has finished.
    #[must_use]
    pub fn info(&self) -> Option<&TlsInfo> {
        match &self.phase {
            Phase::Ready(ready) => Some(&ready.info),
            Phase::Handshake(_) | Phase::Failed => None,
        }
    }

    /// The client Finished verify_data: SMP's session id.
    #[must_use]
    pub fn tls_unique(&self) -> Option<&[u8; 32]> {
        self.info().map(TlsInfo::tls_unique)
    }

    /// DER certificates the server sent, leaf first; empty before the handshake finishes.
    #[must_use]
    pub fn peer_cert_chain(&self) -> &[Vec<u8>] {
        self.info().map_or(&[], TlsInfo::peer_cert_chain)
    }

    /// Takes the application data decrypted so far.
    pub fn read_app(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.app)
    }

    /// Whether the server sent close_notify; data before it has been kept for `read_app`.
    #[must_use]
    pub fn is_peer_closed(&self) -> bool {
        self.peer_closed
    }

    /// The bytes to send for `data`, after any records still waiting in `take_outgoing`, so a
    /// KeyUpdate answer always goes out before the first record under the new key.
    pub fn write_app(&mut self, data: &[u8]) -> Result<Vec<u8>, TlsError> {
        let Phase::Ready(ready) = &mut self.phase else {
            return Err(TlsError::Protocol(
                "application data before the handshake finished",
            ));
        };
        let mut out: Vec<u8> = self.outgoing.drain(..).flatten().collect();
        out.extend(ready.tx.seal_data(data)?);
        Ok(out)
    }

    /// close_notify, after anything still waiting; empty before the handshake finishes.
    pub fn close(&mut self) -> Result<Vec<u8>, TlsError> {
        let Phase::Ready(ready) = &mut self.phase else {
            return Ok(Vec::new());
        };
        let mut out: Vec<u8> = self.outgoing.drain(..).flatten().collect();
        out.extend(ready.tx.close_notify()?);
        Ok(out)
    }
}
