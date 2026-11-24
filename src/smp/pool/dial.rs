//! Dialling a relay's pipe, within the tunnel's limit, and passing events on to the owner.
use std::sync::Arc;

use crossbeam_channel::Sender;

use crate::smp::address::SmpServer;
use crate::smp::events::{SmpEvent, SmpEventKind};

use super::link::Link;
use super::tunnel_pool::{MAX_NOTES, MAX_PIPES, TunnelPool};
use super::why::ms;

impl TunnelPool {
    pub(super) fn dial(&mut self, key: &SmpServer, now: u64) {
        let open = self.slots.values().filter(|s| s.link.is_some()).count();
        let Some(slot) = self.slots.get_mut(key) else {
            return;
        };
        if open >= MAX_PIPES {
            slot.retry_at = now.saturating_add(ms(self.cfg.reconnect_min));
            if self.notes.len() < MAX_NOTES {
                self.notes.push(format!(
                    "{MAX_PIPES} tunnels are open; {} waits",
                    key.hosts[0]
                ));
            }
            return;
        }
        self.next_pipe += 1;
        let id = self.next_pipe;
        let link = (self.dialer.dial(id))
            .and_then(|pipe| Link::new(id, pipe, &slot.server, &self.cfg, now));
        let mut out = Vec::new();
        match link {
            Ok(l) => slot.link = Some(l),
            Err(e) => slot.dial_failed(e, now, &mut out),
        }
        send_all(&self.events, &slot.server, out);
    }
}

pub(super) fn send_all(events: &Sender<SmpEvent>, server: &Arc<SmpServer>, out: Vec<SmpEventKind>) {
    for kind in out {
        let server = Arc::clone(server);
        // Nobody listens once the agent is gone; the events then have no one to tell.
        let _ = events.send(SmpEvent { server, kind });
    }
}
