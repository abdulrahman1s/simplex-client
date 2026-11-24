//! Which relays a check tests, from the Config (the old codec's relay rules).

use crate::types::Config;

use super::SmpTester;
use crate::engine::agent_port::AgentPort;
use crate::engine::machine::Engine;
use crate::smp::SmpServer;

impl<A: AgentPort> Engine<A> {
    /// The own relays of a Config, and the tester that will ask them.
    pub(crate) fn configure_relays(&mut self, hello: &Config) {
        let mut known: Vec<(String, SmpServer)> = Vec::new();
        for r in hello.relays.iter().filter(|r| r.starts_with("smp://")) {
            match r.parse::<SmpServer>() {
                Ok(s) if !known.iter().any(|(_, k)| *k == s) => known.push((r.clone(), s)),
                Ok(_) | Err(_) => {}
            }
        }
        let rel = &mut self.relays;
        rel.own_hosts = known
            .iter()
            .filter_map(|(_, s)| s.hosts.first().cloned())
            .collect();
        rel.known = known.into_iter().map(|(r, _)| r).collect();
        rel.disabled.clear();
        rel.check = None;
        rel.last_check = None;
        rel.after.clear();
        rel.tester.get_or_insert_with(|| Box::new(SmpTester));
    }
}
