//! Commands of a live session: authorised against it, registered as waiting, packed into blocks (Client.hs `sendProtocolCommands`).
use crate::crypto::random_vec;
use crate::smp::auth::Authorizer;
use crate::smp::block::max_payload;
use crate::smp::error::SmpError;
use crate::smp::flow::{Entry, Pending, Request};
use crate::smp::transmission::{encode_transmission, pack};
use crate::smp::transport::Established;

use super::link::Link;

/// Who waits for an answer: the owner under its tag, or nobody for a keepalive PING.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Waiter {
    Tagged(u64),
    Ping,
}

/// The `Large` length and the batch count around one transmission alone in a block.
const FRAMING: usize = 3;

pub(super) struct Outbox {
    /// At most `ConnConfig::max_pending`; entries leave on their answer, on timeout, or when the session ends.
    pub(super) pending: Pending<Waiter>,
    /// Transmissions not yet packed; each has a pending entry, so `max_pending` bounds this too.
    unsent: Vec<Vec<u8>>,
}

impl Outbox {
    pub(super) fn new(max_pending: usize) -> Self {
        Self {
            pending: Pending::new(max_pending),
            unsent: Vec::new(),
        }
    }

    /// Authorises and registers `r`; it goes out with the next `flush`.
    pub(super) fn prepare(
        &mut self,
        session: &Established,
        r: &Request,
        waiter: Waiter,
        now: u64,
        timeout: u64,
    ) -> Result<(), SmpError> {
        let corr = match &r.corr_id {
            Some(c) => c.clone(),
            None => random_vec(24)?,
        };
        let command = r.command.encode(session.version)?;
        let auth = Authorizer {
            session_id: &session.session_id,
            server_key: &session.server_key,
        };
        let authenticator =
            auth.authenticate(r.auth_key.as_ref(), &corr, &r.entity_id, &command)?;
        let t = encode_transmission(
            session.version,
            &authenticator,
            &corr,
            &r.entity_id,
            &command,
        )?;
        if t.len() + FRAMING > max_payload(session.snd.encrypted()) {
            return Err(SmpError::LargeMsg);
        }
        let entry = Entry {
            reply: waiter,
            entity_id: r.entity_id.clone(),
            sent_at: now,
            deadline: now.saturating_add(timeout),
            is_ping: waiter == Waiter::Ping,
        };
        self.pending.insert(corr, entry)?;
        self.unsent.push(t);
        Ok(())
    }

    /// Packs what is waiting into as few blocks as fit and writes them.
    pub(super) fn flush(
        &mut self,
        session: &mut Established,
        link: &mut Link,
    ) -> Result<(), SmpError> {
        if self.unsent.is_empty() {
            return Ok(());
        }
        let (payloads, _) = pack(
            &std::mem::take(&mut self.unsent),
            max_payload(session.snd.encrypted()),
        );
        // `prepare` refused anything that cannot fit a block alone, so nothing is left out.
        payloads
            .iter()
            .try_for_each(|p| link.write(&session.snd.seal(p)?))
    }

    /// Every waiter, as the session ends.
    pub(super) fn drain(&mut self) -> Vec<Waiter> {
        self.unsent.clear();
        self.pending
            .take_all()
            .into_iter()
            .map(|e| e.reply)
            .collect()
    }
}
