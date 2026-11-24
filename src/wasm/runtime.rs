//! The engine, its agent's tunnel pool and the channels between them, opened from the start options.
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use crossbeam_channel::Receiver;

use super::clock::JsClock;
use super::console::Console;
use super::options::Options;
use super::outcome::Outcome;
use super::shared::Shared;
use super::socket::WsDialer;
use super::sugar::Sugar;
use super::waiters::Waiters;
use crate::agent::client::AgentClient;
use crate::engine::jobs::JobDone;
use crate::engine::log::Log;
use crate::engine::machine::{IDLE, MIN_SLEEP};
use crate::engine::{AgentPort, Clock, Engine, NativeAgent};
use crate::smp::{self, ConnConfig, SharedPool, SmpEvent, SystemClock, TunnelPool};
use crate::store::{Store, StoreError};
use crate::types::Config;

pub(super) struct Core {
    pub(super) engine: Engine<NativeAgent>,
    pub(super) pool: SharedPool,
    pub(super) smp: Receiver<SmpEvent>,
    pub(super) jobs: Receiver<JobDone>,
    pub(super) smp_clock: Arc<dyn smp::Clock>,
    pub(super) waiters: Waiters,
    pub(super) sugar: Sugar,
    pub(super) log: Log,
    pub(super) timeout_ms: i64,
    /// Wall ms at which the engine's own timers are next due.
    pub(super) engine_due: i64,
    /// The store revision JS last heard of through `change`.
    pub(super) seen_rev: u64,
    pub(super) next_tag: u64,
}

impl Core {
    pub(super) fn open(o: &Options, shared: &Rc<Shared>) -> Result<Core, StoreError> {
        let store = match &o.snapshot {
            Some(bytes) => Store::open_snapshot(bytes, &o.key)?,
            None => Store::open_memory(&o.key)?,
        };
        let log = if o.log {
            Log::new(Console::default())
        } else {
            Log::discard()
        };
        let (tx, smp) = crossbeam_channel::unbounded();
        let dialer = WsDialer {
            url: o.tunnel.clone(),
            shared: Rc::downgrade(shared),
        };
        let pool = TunnelPool::new(ConnConfig::default(), o.token.clone(), Box::new(dialer), tx);
        let pool: SharedPool = Rc::new(RefCell::new(pool));
        let smp_clock: Arc<dyn smp::Clock> = Arc::new(SystemClock::new());
        let agent = NativeAgent::with_client(&o.relays, log.clone(), |cfg| {
            let clock = Arc::clone(&smp_clock);
            AgentClient::with_pool(cfg, Rc::clone(&pool), clock, Arc::new(JsClock))
        });
        let mut engine = Engine::new(store, agent, Box::new(JsClock), log.clone());
        let config = Config {
            dir: PathBuf::new(),
            key: o.key.clone(),
            name: o.name.clone(),
            relays: o.relays.clone(),
        };
        engine.start(&config)?;
        let jobs = engine.job_results();
        Ok(Core {
            engine,
            pool,
            smp,
            jobs,
            smp_clock,
            waiters: Waiters::default(),
            sugar: Sugar::default(),
            log,
            timeout_ms: o.timeout_ms,
            engine_due: 0,
            seen_rev: 0,
            next_tag: 0,
        })
    }

    /// How long the timer may wait: the engine's timers, the sessions', the waiting calls'.
    pub(super) fn wait_ms(&self) -> u64 {
        let wall = JsClock.wall_ms();
        let smp_now = self.smp_clock.now_ms();
        let pool = self.pool.try_borrow().ok().and_then(|p| p.next_deadline());
        let waits = [
            Some(self.engine_due - wall),
            pool.map(|d| i64::try_from(d.saturating_sub(smp_now)).unwrap_or(i64::MAX)),
            self.waiters.next_deadline().map(|d| d - wall),
        ];
        let soonest = waits.into_iter().flatten().min().unwrap_or(i64::MAX);
        let floor = i64::try_from(MIN_SLEEP.as_millis()).unwrap_or(10);
        let ceiling = i64::try_from(IDLE.as_millis()).unwrap_or(5000);
        u64::try_from(soonest.clamp(floor, ceiling)).unwrap_or(0)
    }

    /// The network closes and every waiting call fails with `closed`; the store goes with it.
    pub(super) fn close(mut self) -> Outcome {
        self.engine.agent.close();
        Outcome {
            calls: Vec::new(),
            settled: self.waiters.close(),
        }
    }
}
