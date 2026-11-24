//! How a client starts: its folder, name, key, relays, log and timeout.

use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use crossbeam_channel::Receiver;

use super::error::{Error, Result};
use super::handle::Client;
use super::spawn::STORE_FILE;
use crate::engine::NativeAgent;
use crate::engine::agent_port::AgentPort;
use crate::engine::clock::{Clock, SystemClock};
use crate::engine::log::Log;
use crate::limits::is_valid_name;
use crate::types::{Config, ErrorCode, ProxyFallback, ProxyMode, RequestKind};

/// Events kept for `events()` before the oldest are dropped.
pub(super) const EVENTS_CAP: usize = 1024;

/// Made by [`Client::builder`]; [`start`](Self::start) opens the store and connects.
#[must_use]
pub struct ClientBuilder {
    pub(super) config: Config,
    pub(super) log: Option<Log>,
    pub(super) timeout: Duration,
    pub(super) events_cap: usize,
}

impl ClientBuilder {
    pub(super) fn new(dir: PathBuf) -> Self {
        Self {
            config: Config {
                dir,
                key: String::new(),
                name: "SimpleX bot".into(),
                relays: Vec::new(),
                proxy_mode: ProxyMode::default(),
                proxy_fallback: ProxyFallback::default(),
            },
            log: None,
            timeout: Duration::from_secs(60),
            events_cap: EVENTS_CAP,
        }
    }

    /// The display name a new store starts with; an existing store keeps its own.
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.config.name = name.into();
        self
    }

    /// The passphrase that seals the store (empty by default).
    pub fn key(mut self, key: impl Into<String>) -> Self {
        self.config.key = key.into();
        self
    }

    /// Adds a relay (`smp://…` or `xftp://…`). With none, the protocol's preset relays are used.
    pub fn relay(mut self, address: impl Into<String>) -> Self {
        self.config.relays.push(address.into());
        self
    }

    /// When messages go through one of our relays instead of straight to the recipient's,
    /// which would see our IP address ([`ProxyMode::Unknown`] by default: for relays not ours).
    pub fn private_routing(mut self, mode: ProxyMode) -> Self {
        self.config.proxy_mode = mode;
        self
    }

    /// Whether a message may go straight to the recipient's relay when private routing to it
    /// fails (no by default).
    pub fn allow_direct(mut self, allow: bool) -> Self {
        self.config.proxy_fallback = if allow {
            ProxyFallback::Allow
        } else {
            ProxyFallback::AllowProtected
        };
        self
    }

    /// Where the engine logs, one line per entry (standard error by default).
    pub fn log(mut self, sink: impl Write + Send + 'static) -> Self {
        self.log = Some(Log::new(sink));
        self
    }

    /// How long a method that waits for an answer waits (60 seconds by default).
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Opens the store, starts the engine and waits until it is ready.
    pub fn start(self) -> Result<Client> {
        self.start_with(Box::new(SystemClock), |config, log| {
            NativeAgent::new(&config.relays, log.clone())
        })
    }

    /// `agent` makes the agent on the engine thread, with the channel its network sends on.
    pub(crate) fn start_with<A, F>(self, clock: Box<dyn Clock>, agent: F) -> Result<Client>
    where
        A: AgentPort + 'static,
        F: FnOnce(&Config, &Log) -> (A, Receiver<A::Smp>) + Send + 'static,
    {
        // Only a new store's user takes this name (Commands.hs `CreateActiveUser`:
        // `checkValidName`); an existing store keeps its own.
        let new_store = !self.config.dir.join(STORE_FILE).exists();
        if new_store && !is_valid_name(self.config.name.trim()) {
            return Err(Error::Failed {
                request: RequestKind::Start,
                code: ErrorCode::InvalidName,
                detail: format!("not a valid name: {:?}", self.config.name),
            });
        }
        Client::start(self, clock, agent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routing_settings_reach_the_config_and_default_to_the_chats() {
        let b = ClientBuilder::new(PathBuf::from("x"));
        assert_eq!(b.config.proxy_mode, ProxyMode::Unknown);
        assert_eq!(b.config.proxy_fallback, ProxyFallback::AllowProtected);
        let b = b.private_routing(ProxyMode::Always).allow_direct(true);
        assert_eq!(b.config.proxy_mode, ProxyMode::Always);
        assert_eq!(b.config.proxy_fallback, ProxyFallback::Allow);
    }
}
