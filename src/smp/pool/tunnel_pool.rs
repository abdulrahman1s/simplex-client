//! The relays reached through the tunnel: one slot per server, one pipe and session per slot.
use std::collections::HashMap;
use std::sync::Arc;

use crossbeam_channel::Sender;

use crate::smp::address::SmpServer;
use crate::smp::config::ConnConfig;
use crate::smp::error::SmpError;
use crate::smp::events::{SmpEvent, SmpEventKind};
use crate::smp::flow::Request;

use super::dial::send_all;
use super::pipe::{Dialer, PipeId, PipeInput};
use super::slot::Slot;

/// The Worker allows this many tunnels open at once per address.
pub const MAX_PIPES: usize = 8;

/// Lines kept for the owner's log until it takes them; more are dropped.
pub(super) const MAX_NOTES: usize = 32;

/// Sans-I/O apart from the pipes it dials: every call takes `now` (ms of an `smp::Clock`), and
/// events go out on `events` exactly as `SmpConnection` sends them.
pub struct TunnelPool {
    pub(super) cfg: ConnConfig,
    pub(super) token: Option<String>,
    pub(super) dialer: Box<dyn Dialer>,
    pub(super) events: Sender<SmpEvent>,
    /// Eviction: `retain` drops the servers no queue names; at most `MAX_PIPES` have a pipe.
    pub(super) slots: HashMap<SmpServer, Slot>,
    pub(super) next_pipe: PipeId,
    /// Bounded by `MAX_NOTES`; emptied by `take_notes`.
    pub(super) notes: Vec<String>,
}

impl TunnelPool {
    #[must_use]
    pub fn new(
        cfg: ConnConfig,
        token: Option<String>,
        dialer: Box<dyn Dialer>,
        events: Sender<SmpEvent>,
    ) -> Self {
        Self {
            cfg,
            token,
            dialer,
            events,
            slots: HashMap::new(),
            next_pipe: 0,
            notes: Vec::new(),
        }
    }

    /// Starts dialling `server` unless it has a slot already; Up arrives as an event.
    pub fn ensure(&mut self, server: &SmpServer, now: u64) {
        if !self.slots.contains_key(server) {
            let slot = Slot::new(server.clone(), &self.cfg, now);
            self.slots.insert(server.clone(), slot);
            self.dial(server, now);
        }
    }

    /// Like `SmpConnection::send_tagged`: `NotConnected` unless the session is up.
    pub fn send(
        &mut self,
        server: &SmpServer,
        request: Request,
        tag: u64,
        now: u64,
    ) -> Result<(), SmpError> {
        self.ensure(server, now);
        let link = self.slots.get_mut(server).and_then(|s| s.link.as_mut());
        match link {
            Some(l) if l.session.is_up() => l.session.send(request, tag, now),
            Some(_) | None => Err(SmpError::NotConnected),
        }
    }

    /// Closes the servers `keep` refuses; like a closed `SmpConnection` they report nothing more.
    pub fn retain(&mut self, keep: impl Fn(&SmpServer) -> bool) {
        self.slots.retain(|server, slot| {
            let kept = keep(server);
            if let (false, Some(link)) = (kept, &mut slot.link) {
                link.pipe.close();
            }
            kept
        });
    }

    pub fn close_all(&mut self) {
        self.retain(|_| false);
    }

    /// What pipe `id` reported. News of a pipe the pool has replaced is ignored.
    pub fn on_input(&mut self, id: PipeId, input: PipeInput, now: u64) {
        let token = self.token.as_deref();
        let found = (self.slots.values_mut()).find(|s| s.link.as_ref().is_some_and(|l| l.id == id));
        let Some(slot) = found else { return };
        let mut out = Vec::new();
        slot.on_input(input, now, token, &mut out, &mut self.notes);
        let server = Arc::clone(&slot.server);
        self.emit(&server, out);
    }

    /// Session timers, then dials that are due.
    pub fn tick(&mut self, now: u64) {
        let mut due = Vec::new();
        for (key, slot) in &mut self.slots {
            let mut out = Vec::new();
            slot.tick(now, &mut out);
            let server = Arc::clone(&slot.server);
            if slot.link.is_none() && slot.retry_at <= now {
                due.push(key.clone());
            }
            send_all(&self.events, &server, out);
        }
        for key in due {
            self.dial(&key, now);
        }
    }

    /// Sends what the sessions have for their pipes; call after every input.
    pub fn flush(&mut self, now: u64) {
        for slot in self.slots.values_mut() {
            let mut out = Vec::new();
            slot.flush(now, &mut out);
            send_all(&self.events, &slot.server, out);
        }
    }

    /// When `tick` next has work, if ever.
    #[must_use]
    pub fn next_deadline(&self) -> Option<u64> {
        let each = self.slots.values().map(|s| match &s.link {
            Some(l) => l.session.next_deadline(),
            None => Some(s.retry_at),
        });
        each.flatten().min()
    }

    /// Lines for the owner's log since the last call.
    pub fn take_notes(&mut self) -> Vec<String> {
        std::mem::take(&mut self.notes)
    }

    #[must_use]
    pub fn is_up(&self, server: &SmpServer) -> bool {
        let link = self.slots.get(server).and_then(|s| s.link.as_ref());
        link.is_some_and(|l| l.session.is_up())
    }

    fn emit(&mut self, server: &Arc<SmpServer>, out: Vec<SmpEventKind>) {
        self.notes.truncate(MAX_NOTES);
        send_all(&self.events, server, out);
    }
}
