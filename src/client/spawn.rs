//! The engine thread: the store, the agent and the engine are made on it and never leave it.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;
use std::thread::{self, JoinHandle};

use crossbeam_channel::{Receiver, Sender, bounded};

use super::error::{Error, Result};
use crate::engine::agent_port::AgentPort;
use crate::engine::clock::Clock;
use crate::engine::log::Log;
use crate::engine::machine::Engine;
use crate::store::Store;
use crate::types::{Config, Event, Request};

/// The store's file in the client's folder.
pub(super) const STORE_FILE: &str = "chat.db";

/// Locked while a client has the folder's store open, so a second one fails instead of
/// writing the same store; a separate file, so SQLite's own locks are left alone.
const LOCK_FILE: &str = "chat.db.lock";

/// Events the engine thread may get ahead of the dispatcher by; the engine's own outbox
/// (`machine::OUT_CAP`) holds the rest.
const ENGINE_EVENTS: usize = 256;

/// A started engine: what it said while starting, and its channels.
pub(super) struct Started {
    pub(super) requests: Sender<Request>,
    pub(super) events: Receiver<Event>,
    pub(super) startup: Vec<Event>,
    pub(super) thread: JoinHandle<()>,
}

/// Starts the engine on its own thread and waits until it is ready or failed.
pub(super) fn spawn<A, F>(
    config: Config,
    log: Log,
    clock: Box<dyn Clock>,
    agent: F,
) -> Result<Started>
where
    A: AgentPort + 'static,
    F: FnOnce(&Config, &Log) -> (A, Receiver<A::Smp>) + Send + 'static,
{
    // Unbounded: the engine stops taking requests only while its outbox is full, and the
    // dispatcher always empties that, so requests wait here no longer than one at a time.
    let (requests, requests_rx) = crossbeam_channel::unbounded();
    let (events_tx, events) = bounded(ENGINE_EVENTS);
    let (ready_tx, ready) = bounded(1);
    let thread = thread::Builder::new()
        .name("simplex-engine".into())
        .spawn(move || {
            // The lock goes when this thread ends, so `close()` has released it on return.
            let (mut engine, smp, _lock) = match open(&config, &log, clock, agent) {
                Ok(started) => started,
                Err(e) => {
                    // The builder waits on this; if it is gone there is nobody to tell.
                    let _ = ready_tx.send(Err(e));
                    return;
                }
            };
            let startup = std::iter::from_fn(|| engine.pop_out()).collect();
            if ready_tx.send(Ok(startup)).is_err() {
                return;
            }
            crate::engine::run::run(engine, &requests_rx, smp, &events_tx);
        })?;
    let startup = match ready.recv() {
        Ok(Ok(startup)) => startup,
        Ok(Err(e)) => {
            let _ = thread.join(); // it has returned already; the error is what matters
            return Err(e);
        }
        Err(_) => {
            let _ = thread.join(); // it panicked; `Closed` says the engine is gone
            return Err(Error::Closed);
        }
    };
    Ok(Started {
        requests,
        events,
        startup,
        thread,
    })
}

fn open<A, F>(
    config: &Config,
    log: &Log,
    clock: Box<dyn Clock>,
    agent: F,
) -> Result<(Engine<A>, Receiver<A::Smp>, File)>
where
    A: AgentPort,
    F: FnOnce(&Config, &Log) -> (A, Receiver<A::Smp>),
{
    std::fs::create_dir_all(&config.dir)?;
    let lock = lock(&config.dir)?;
    let store = Store::open(&config.dir.join(STORE_FILE), &config.key)?;
    let (agent, smp) = agent(config, log);
    let mut engine = Engine::new(store, agent, clock, log.clone());
    engine.start(config)?;
    Ok((engine, smp, lock))
}

/// An exclusive lock on the folder's lock file, released when the file is closed.
fn lock(dir: &Path) -> Result<File> {
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(dir.join(LOCK_FILE))?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(Error::Locked(dir.to_owned())),
        Err(TryLockError::Error(e)) => Err(e.into()),
    }
}
