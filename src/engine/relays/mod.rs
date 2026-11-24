//! Relay checks: which of the relays in use answer, which take new queues, and the one retry after a network failure (the old codec's relay rules; Agent.hs `testProtocolServer`).

mod check;
mod configure;
mod real;
mod retry;
mod tester;
mod wake;

use std::collections::{BTreeSet, HashSet};
use std::time::Duration;

use check::Check;
pub use real::SmpTester;
pub(crate) use retry::Retry;
pub use tester::{RelayDone, RelayTester};

/// The least time between two checks; a failure sooner is just sent again.
pub const CHECK_EVERY: Duration = Duration::from_secs(180);

/// A check that has not heard from every relay by then counts the silent ones as failed.
pub const CHECK_DEADLINE: Duration = Duration::from_secs(90);

#[derive(Default)]
pub struct Relays {
    /// Injected before `start` by tests; the start takes the agent's when none is there.
    pub(crate) tester: Option<Box<dyn RelayTester>>,
    /// The relays checked: the user's own SMP relays from the config, or the default presets.
    /// Bounded by the config or the presets.
    known: Vec<String>,
    /// Their hosts, the only ones whose Up and Down decide Offline; empty means every host.
    pub(crate) own_hosts: HashSet<String>,
    /// The ones the last check found silent, while another answered. Subset of `known`.
    disabled: BTreeSet<String>,
    check: Option<Check>,
    /// Wall ms the last check started.
    last_check: Option<i64>,
    run: u64,
    /// Requests waiting for the running check. At most one of each kind.
    after: Vec<Retry>,
    /// The connection of the retry of each kind: its failure is reported, not retried again.
    retried: [Option<super::agent_port::ConnId>; 2],
}
