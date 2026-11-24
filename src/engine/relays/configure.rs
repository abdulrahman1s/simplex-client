//! Which relays a check tests: the Config's own, or the presets in use when it names none (the old codec's relay rules, `NativeAgent::new`).

use crate::types::Config;

use crate::engine::agent_port::AgentPort;
use crate::engine::machine::Engine;
use crate::engine::presets;
use crate::smp::SmpServer;

impl<A: AgentPort> Engine<A> {
    /// The relays the agent creates queues on, and the tester that will ask them. Named relays
    /// that do not parse leave none, as the agent then has none either.
    pub(crate) fn configure_relays(&mut self, hello: &Config) {
        let named: Vec<&str> = hello
            .relays
            .iter()
            .map(String::as_str)
            .filter(|r| r.starts_with("smp://"))
            .collect();
        let own = !named.is_empty();
        // Only the 7 default presets take queues, not all 20; 7.1.4 tests servers only on request.
        let candidates: Vec<&str> = if own {
            named
        } else {
            presets::default_addresses().collect()
        };
        let mut known: Vec<(String, SmpServer)> = Vec::new();
        for r in candidates {
            match r.parse::<SmpServer>() {
                Ok(s) if !known.iter().any(|(_, k)| *k == s) => known.push((r.to_owned(), s)),
                Ok(_) | Err(_) => {}
            }
        }
        let rel = &mut self.relays;
        // Offline from the agent's hosts stays on every host with the presets: a peer's relay counts.
        rel.own_hosts = known
            .iter()
            .filter(|_| own)
            .filter_map(|(_, s)| s.hosts.first().cloned())
            .collect();
        rel.known = known.into_iter().map(|(r, _)| r).collect();
        rel.disabled.clear();
        rel.check = None;
        rel.last_check = None;
        rel.after.clear();
        if rel.tester.is_none() {
            rel.tester = self.agent.relay_tester();
        }
    }
}
