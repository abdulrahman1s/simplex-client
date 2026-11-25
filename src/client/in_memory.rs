//! A client on the in-memory SMP server of `agent::testing`, for end-to-end tests without a network (feature `testing`).

use std::sync::Arc;

use crossbeam_channel::unbounded;

use super::builder::ClientBuilder;
use super::error::Result;
use super::handle::Client;
use crate::agent::client::{AgentClient, SystemWallClock};
use crate::agent::testing::{FakeConnector, FakeSmp};
use crate::engine::{NativeAgent, SystemClock};

impl ClientBuilder {
    /// Starts on `server` alone, an in-memory SMP server the clients of a test share:
    /// connections, messages, groups and everything else but files (there is no XFTP
    /// server), with the real agent, engine and store and no network: relays are not
    /// checked, and no XFTP relay is used. The relays given before are replaced by `server`.
    pub fn start_in_memory(mut self, server: &Arc<FakeSmp>) -> Result<Client> {
        self.config.relays = vec![server.address().to_string()];
        let server = Arc::clone(server);
        self.start_with(Box::new(SystemClock), move |config, log| {
            let (tx, rx) = unbounded();
            let agent = NativeAgent::with_client(&config.relays, log.clone(), |cfg| {
                AgentClient::with_parts(
                    cfg,
                    tx,
                    Arc::new(FakeConnector(server)),
                    Arc::new(crate::smp::SystemClock::new()),
                    Arc::new(SystemWallClock),
                )
            });
            // A check would dial the server's made-up host over the network.
            (agent.without_relay_tests(), rx)
        })
    }
}
