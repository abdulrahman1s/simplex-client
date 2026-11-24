//! A check: every own relay tested at once, the silent ones kept off new queues while another answers, and none answering is the network gone.

use std::collections::HashSet;

use super::{CHECK_DEADLINE, CHECK_EVERY, RelayDone};
use crate::engine::agent_port::AgentPort;
use crate::engine::machine::Engine;

pub(super) struct Check {
    run: u64,
    waiting: HashSet<String>,
    passed: HashSet<String>,
    /// Wall ms after which the silent are counted as failed.
    pub(super) deadline: i64,
}

pub(super) fn ms(d: std::time::Duration) -> i64 {
    i64::try_from(d.as_millis()).unwrap_or(i64::MAX)
}

impl<A: AgentPort> Engine<A> {
    /// Starts a check unless one runs, one started less than `CHECK_EVERY` ago, or no relay is
    /// known. A lone relay is checked only for the network: it is never disabled. Whether one started.
    pub(crate) fn start_check(&mut self) -> bool {
        let now = self.clock.wall_ms();
        let tx = self.jobs.tx.clone();
        let rel = &mut self.relays;
        let recent = rel.last_check.is_some_and(|l| now - l < ms(CHECK_EVERY));
        if rel.check.is_some() || recent || rel.known.is_empty() {
            return false;
        }
        let Some(tester) = rel.tester.as_mut() else {
            return false;
        };
        rel.run += 1;
        rel.last_check = Some(now);
        for server in &rel.known {
            tester.test(server.clone(), rel.run, tx.clone());
        }
        rel.check = Some(Check {
            run: rel.run,
            waiting: rel.known.iter().cloned().collect(),
            passed: HashSet::new(),
            deadline: now + ms(CHECK_DEADLINE),
        });
        true
    }

    pub(crate) fn on_relay_done(&mut self, done: RelayDone) {
        let Some(c) = self.relays.check.as_mut().filter(|c| c.run == done.run) else {
            return;
        };
        if !c.waiting.remove(&done.server) {
            return;
        }
        match done.failure {
            None => {
                c.passed.insert(done.server);
            }
            Some(why) => self.log.line(format_args!(
                "relay {} did not answer its test: {why}",
                host_of(&done.server)
            )),
        }
        if self
            .relays
            .check
            .as_ref()
            .is_some_and(|c| c.waiting.is_empty())
        {
            self.finish_check();
        }
    }

    /// Every answer is in, or the deadline passed: disable what stayed silent and enable what
    /// answered, unless nothing answered (then the network is down, not the relays).
    pub(super) fn finish_check(&mut self) {
        let Some(c) = self.relays.check.take() else {
            return;
        };
        if c.passed.is_empty() {
            self.log
                .line(format_args!("no relay answered; none is disabled"));
            // The agent ignores a connection that never came up, so at a start without a network
            // this is the only sign; a host the agent still has up says the network is there.
            if !self.state.hosts.values().any(|up| *up) {
                self.set_offline(true);
            }
        } else {
            self.set_offline(false);
            let silent = self
                .relays
                .known
                .iter()
                .filter(|r| !c.passed.contains(*r))
                .cloned()
                .collect();
            self.set_disabled(silent);
        }
        self.flush_retries();
    }

    fn set_disabled(&mut self, silent: std::collections::BTreeSet<String>) {
        if silent == self.relays.disabled {
            return;
        }
        for r in silent.symmetric_difference(&self.relays.disabled) {
            let on = !silent.contains(r);
            self.log.line(format_args!(
                "relay {} is {} for new queues",
                host_of(r),
                if on { "enabled" } else { "disabled" }
            ));
        }
        let newly = silent.difference(&self.relays.disabled).next().is_some();
        self.relays.disabled = silent;
        let list: Vec<String> = self.relays.disabled.iter().cloned().collect();
        self.agent.set_disabled_relays(&list);
        if newly {
            self.restart_pending_queues();
        }
    }
}

/// A relay's host, for the log (no key fingerprint or password).
fn host_of(server: &str) -> &str {
    server.rsplit('@').next().unwrap_or(server)
}
