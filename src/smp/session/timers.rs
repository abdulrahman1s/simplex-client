//! Time in: command timeouts, the keepalive PING and the judgement of silence (Client.hs `pingServer`).
use crate::smp::commands::Command;
use crate::smp::error::SmpError;
use crate::smp::events::SmpEventKind;
use crate::smp::flow::{Request, Verdict};

use super::machine::{Live, SmpSession, State, ms};
use super::outbox::{Outbox, Waiter};

impl SmpSession {
    /// Runs what is due at `now_ms`; call it by `next_deadline`.
    pub fn tick(&mut self, now_ms: u64) -> Vec<SmpEventKind> {
        let mut events = Vec::new();
        if let Some(e) = self.broken.take() {
            self.go_down(e, &mut events);
            return events;
        }
        let periods = (ms(self.cfg.ping_interval), ms(self.cfg.command_timeout));
        let due = match &mut self.state {
            State::Down => Ok(()),
            State::Connecting { deadline } if now_ms >= *deadline => Err(SmpError::Timeout),
            State::Connecting { .. } => Ok(()),
            State::Up(live) => tick_up(live, &mut self.outbox, periods, now_ms, &mut events),
        };
        if let Err(e) = due {
            self.go_down(e, &mut events);
        }
        events
    }

    /// When `tick` next has work, if ever.
    #[must_use]
    pub fn next_deadline(&self) -> Option<u64> {
        if self.broken.is_some() {
            return Some(0);
        }
        match &self.state {
            State::Down => None,
            State::Connecting { deadline } => Some(*deadline),
            State::Up(live) => [self.outbox.pending.next_deadline(), live.next_ping]
                .into_iter()
                .flatten()
                .min(),
        }
    }
}

/// Expires waiting commands, then sends a PING when one is due or a late timer calls for a probe.
fn tick_up(
    live: &mut Live,
    outbox: &mut Outbox,
    (ping_every, timeout): (u64, u64),
    now: u64,
    events: &mut Vec<SmpEventKind>,
) -> Result<(), SmpError> {
    let (mut probe, mut dead) = (false, false);
    // Every expired command is answered, even after a verdict of dead: each tag gets exactly one reply.
    for e in outbox.pending.expire(now) {
        if let Waiter::Tagged(tag) = e.reply {
            events.push(SmpEventKind::Reply {
                tag,
                answer: Err(SmpError::Timeout),
            });
        }
        match live.liveness.on_timeout(e.sent_at, e.deadline, now) {
            Verdict::Dead => dead = true,
            Verdict::Probe => probe = true,
            Verdict::Alive => {}
        }
    }
    if dead {
        return Err(SmpError::Unresponsive);
    }
    let ping_due = live.next_ping.is_some_and(|t| now >= t);
    if (ping_due || probe) && !outbox.pending.has_ping() {
        let ping = Request::new(Command::Ping, Vec::new(), None);
        outbox.prepare(&live.session, &ping, Waiter::Ping, now, timeout)?;
    }
    if ping_due {
        live.next_ping = Some(now.saturating_add(ping_every));
    }
    Ok(())
}
