//! The check's timer: the deadline of a running check, and the next check while a relay is disabled or the network is gone.

use super::CHECK_EVERY;
use super::check::ms;
use crate::engine::agent_port::AgentPort;
use crate::engine::machine::Engine;

impl<A: AgentPort> Engine<A> {
    /// While offline the checks go on, since a user with no connection gets no `Up` to end it.
    #[must_use]
    pub(crate) fn next_relay_wake(&self) -> Option<i64> {
        let rel = &self.relays;
        let watching = !rel.disabled.is_empty() || self.state.offline;
        match (&rel.check, watching) {
            (Some(c), _) => Some(c.deadline),
            (None, true) => rel.last_check.map(|l| l + ms(CHECK_EVERY)),
            (None, false) => None,
        }
    }

    pub(crate) fn relay_tick(&mut self, now: i64) {
        if self.next_relay_wake().is_none_or(|at| at > now) {
            return;
        }
        if self.relays.check.is_some() {
            self.finish_check();
        } else {
            self.start_check();
        }
    }
}
