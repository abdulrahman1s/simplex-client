//! Bytes in: the server hello settles the session, later blocks carry answers and pushes (Client.hs `receiveSMP`).
use crate::smp::error::SmpError;
use crate::smp::events::SmpEventKind;
use crate::smp::flow::{Liveness, route};
use crate::smp::handshake::{HandshakeConfig, TlsFacts, answer_server_hello};
use crate::smp::responses::Response;
use crate::smp::transmission::parse_payload;

use super::machine::{Live, SmpSession, State, info, ms};
use super::outbox::Waiter;

impl SmpSession {
    /// Takes bytes from the network and returns what they meant, in order.
    pub fn feed(&mut self, bytes: &[u8], now_ms: u64) -> Vec<SmpEventKind> {
        let mut events = Vec::new();
        if matches!(self.state, State::Down) {
            return events;
        }
        if let Some(e) = self.broken.take() {
            self.go_down(e, &mut events);
            return events;
        }
        if let Err(e) = self.receive(bytes, now_ms, &mut events) {
            self.go_down(e, &mut events);
        } else if self.link.is_peer_closed() {
            self.go_down(SmpError::Closed, &mut events);
        }
        events
    }

    fn receive(
        &mut self,
        bytes: &[u8],
        now: u64,
        events: &mut Vec<SmpEventKind>,
    ) -> Result<(), SmpError> {
        for block in self.link.feed(bytes)? {
            match &self.state {
                State::Connecting { .. } => self.handshake(&block, now, events)?,
                State::Up(_) => self.on_block(&block, now, events)?,
                State::Down => {}
            }
        }
        Ok(())
    }

    fn handshake(
        &mut self,
        block: &[u8],
        now: u64,
        events: &mut Vec<SmpEventKind>,
    ) -> Result<(), SmpError> {
        let tls = self.link.tls();
        let unique = tls
            .tls_unique()
            .ok_or(SmpError::Unexpected("block before TLS"))?;
        let facts = TlsFacts {
            tls_unique: unique,
            peer_chain: tls.peer_cert_chain(),
        };
        let cfg = HandshakeConfig {
            range: self.cfg.versions,
            block_encryption: self.cfg.block_encryption,
            timeout: self.cfg.connect_timeout,
        };
        let (hello, session) = answer_server_hello(block, &facts, &self.key_hash, &cfg)?;
        self.link.write(&hello)?;
        events.push(SmpEventKind::Up(info(&session)));
        let ping = ms(self.cfg.ping_interval);
        let liveness = Liveness::new(self.cfg.max_timeouts, ms(self.cfg.late_after), now);
        self.state = State::Up(Box::new(Live {
            session,
            liveness,
            next_ping: (ping > 0).then(|| now.saturating_add(ping)),
        }));
        let queued: Vec<_> = self.queued.drain(..).collect();
        for (request, tag) in queued {
            if let Err(e) = self.send(request, tag, now) {
                events.push(SmpEventKind::Reply {
                    tag,
                    answer: Err(e),
                });
            }
        }
        Ok(())
    }

    fn on_block(
        &mut self,
        block: &[u8],
        now: u64,
        events: &mut Vec<SmpEventKind>,
    ) -> Result<(), SmpError> {
        let State::Up(live) = &mut self.state else {
            return Ok(());
        };
        live.liveness.observe_rx(now);
        let version = live.session.version;
        let payload = live.session.rcv.open(block)?;
        for t in parse_payload(version, &payload)? {
            let decoded = Response::decode(version, &t.command);
            let waiting = if t.corr_id.is_empty() {
                None
            } else {
                self.outbox.pending.take(&t.corr_id)
            };
            let waited = waiting.as_ref().map(|e| e.entity_id.as_slice());
            let routed = route(decoded, t.entity_id, waited);
            events.extend(routed.event);
            if let (Some(entry), Some(answer)) = (waiting, routed.answer)
                && let Waiter::Tagged(tag) = entry.reply
            {
                events.push(SmpEventKind::Reply { tag, answer });
            }
        }
        Ok(())
    }
}
