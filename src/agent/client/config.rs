//! The agent's servers, timings and retry intervals (Env/SQLite.hs `defaultAgentConfig`, simplexmq 7.1.4, AGPL-3.0-only).
use std::time::Duration;

use crate::agent::version::supported_agent_range;
use crate::encoding::VersionRange;
use crate::smp::{ConnConfig, SmpServer};
use crate::types::{ProxyFallback, ProxyMode};

/// `RetryInterval`: the delay stays at `initial` until `increase_after` has passed, then grows by half up to `max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryInterval {
    pub initial: Duration,
    pub increase_after: Duration,
    pub max: Duration,
}

#[derive(Debug, Clone)]
pub struct DeliveryConfig {
    /// `messageRetryInterval.riFast`.
    pub fast: RetryInterval,
    /// `riSlow`, used after QUOTA.
    pub slow: RetryInterval,
    /// `messageTimeout`: how long a message is retried after temporary failures.
    pub message_timeout: Duration,
    /// `quotaExceededTimeout`: how long a message waits for room in a full queue.
    pub quota_timeout: Duration,
}

impl Default for DeliveryConfig {
    fn default() -> Self {
        Self {
            fast: RetryInterval {
                initial: Duration::from_secs(2),
                increase_after: Duration::from_secs(10),
                max: Duration::from_secs(120),
            },
            slow: RetryInterval {
                initial: Duration::from_secs(300),
                increase_after: Duration::from_secs(60),
                max: Duration::from_secs(6 * 3600),
            },
            message_timeout: Duration::from_secs(2 * 24 * 3600),
            quota_timeout: Duration::from_secs(7 * 24 * 3600),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AgentConfig {
    /// Servers our receive queues are created on; the first is used. Also the proxies, and what
    /// makes a recipient's relay "known" (`UserServers`).
    pub servers: Vec<SmpServer>,
    /// Those of `servers` the user turned off: not proxies, unless no other is left (`ServerCfg.enabled`).
    pub disabled: Vec<SmpServer>,
    /// The built-in servers, which short links may name by host only.
    pub presets: Vec<SmpServer>,
    pub smp: ConnConfig,
    pub delivery: DeliveryConfig,
    /// How soon a handshake step (NEW, SKEY, LKEY) that failed is sent again while its server is up.
    pub step_retry: Duration,
    /// `smpAgentVRange`: the agent versions links offer and joins accept; every message carries its maximum.
    pub agent_versions: VersionRange,
    /// `smpProxyMode`.
    pub proxy_mode: ProxyMode,
    /// `smpProxyFallback`.
    pub proxy_fallback: ProxyFallback,
}

impl AgentConfig {
    #[must_use]
    pub fn new(servers: Vec<SmpServer>) -> Self {
        Self {
            servers,
            disabled: Vec::new(),
            presets: Vec::new(),
            smp: ConnConfig::default(),
            delivery: DeliveryConfig::default(),
            step_retry: Duration::from_secs(5),
            agent_versions: supported_agent_range(),
            // The agent's own `defaultNetworkConfig`; the chat sets its defaults through `types::Config`.
            proxy_mode: ProxyMode::Never,
            proxy_fallback: ProxyFallback::Allow,
        }
    }
}

pub(crate) fn ms(d: Duration) -> i64 {
    i64::try_from(d.as_millis()).unwrap_or(i64::MAX)
}
