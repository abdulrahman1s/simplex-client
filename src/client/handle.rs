//! The client handle: cheap to clone, shared between threads, closed when the last clone drops.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam_channel::{Receiver, RecvTimeoutError, Sender, bounded};

use super::builder::ClientBuilder;
use super::dispatch::{self, Outbox, Shared};
use super::error::{Error, Result};
use super::spawn;
use super::waiters::Want;
use crate::engine::agent_port::AgentPort;
use crate::engine::clock::Clock;
use crate::engine::log::Log;
use crate::types::model::ChatProfile;
use crate::types::{Config, Event, Request};

/// A running SimpleX client. Clones share one engine; methods may be called from any thread.
#[derive(Clone)]
pub struct Client {
    inner: Arc<Inner>,
}

struct Inner {
    requests: Mutex<Option<Sender<Request>>>,
    events: Receiver<Event>,
    shared: Arc<Shared>,
    /// The engine thread, then the dispatcher: joined in that order.
    threads: Mutex<Vec<JoinHandle<()>>>,
    timeout: Duration,
    tags: AtomicU64,
    log: Log,
}

impl Client {
    /// A client whose store and received files live in `dir` (made if missing).
    pub fn builder(dir: impl Into<PathBuf>) -> ClientBuilder {
        ClientBuilder::new(dir.into())
    }

    pub(super) fn start<A, F>(b: ClientBuilder, clock: Box<dyn Clock>, agent: F) -> Result<Self>
    where
        A: AgentPort + 'static,
        F: FnOnce(&Config, &Log) -> (A, Receiver<A::Smp>) + Send + 'static,
    {
        let log = b.log.unwrap_or_else(|| Log::new(std::io::stderr()));
        let started = spawn::spawn(b.config, log.clone(), clock, agent)?;
        let shared = Arc::new(Shared::default());
        let (tx, events) = bounded(b.events_cap.max(1));
        let outbox = Outbox {
            tx,
            rx: events.clone(),
            log: log.clone(),
        };
        for e in &started.startup {
            shared.note(e);
        }
        let dispatcher =
            dispatch::spawn(started.events, started.startup, Arc::clone(&shared), outbox)?;
        Ok(Self {
            inner: Arc::new(Inner {
                requests: Mutex::new(Some(started.requests)),
                events,
                shared,
                threads: Mutex::new(vec![started.thread, dispatcher]),
                timeout: b.timeout,
                tags: AtomicU64::new(1),
                log,
            }),
        })
    }

    /// Sends any request as it is; what it causes arrives as events. The escape hatch for
    /// everything without its own method (calls, streams, muting, pictures, …).
    pub fn request(&self, request: Request) -> Result<()> {
        let tx = self
            .inner
            .requests
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
            .ok_or(Error::Closed)?;
        tx.send(request).map_err(|_| Error::Closed)
    }

    /// Everything that happens, in order, blocking until the next event; ends when the client
    /// closes. Clones share one queue: each event goes to one reader. Answers to waiting
    /// methods are not repeated here. Up to 1024 unread events are kept, then the oldest go.
    pub fn events(&self) -> impl Iterator<Item = Event> + Send + 'static {
        self.inner.events.clone().into_iter()
    }

    /// The next event, waiting at most `timeout`.
    pub fn next_event(&self, timeout: Duration) -> Result<Event> {
        self.inner
            .events
            .recv_timeout(timeout)
            .map_err(|e| match e {
                RecvTimeoutError::Timeout => Error::Timeout,
                RecvTimeoutError::Disconnected => Error::Closed,
            })
    }

    /// The user's profile as the engine last reported it.
    #[must_use]
    pub fn profile(&self) -> ChatProfile {
        let p = self.inner.shared.profile.lock();
        p.unwrap_or_else(PoisonError::into_inner).clone()
    }

    pub(super) fn remember_profile(&self, profile: &ChatProfile) {
        let p = self.inner.shared.profile.lock();
        profile.clone_into(&mut p.unwrap_or_else(PoisonError::into_inner));
    }

    /// Stops the engine for every clone: the network closes, the store is saved and closed,
    /// waiting calls fail with `Closed` and `events()` ends once drained.
    pub fn close(&self) {
        self.inner.shutdown();
    }

    /// Sends `request` and waits for the event that answers it.
    pub(super) fn ask(&self, want: Want, request: Request) -> Result<Event> {
        let waiters = &self.inner.shared.waiters;
        waiters.wait(want, self.inner.timeout, || self.request(request))
    }

    pub(super) fn next_tag(&self) -> u64 {
        self.inner.tags.fetch_add(1, Ordering::Relaxed)
    }
}

impl Inner {
    fn shutdown(&self) {
        drop(
            self.requests
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .take(),
        );
        let threads =
            std::mem::take(&mut *self.threads.lock().unwrap_or_else(PoisonError::into_inner));
        for t in threads {
            if t.join().is_err() {
                self.log.line(format_args!("a client thread panicked"));
            }
        }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.shutdown();
    }
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Client")
    }
}
