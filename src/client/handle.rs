//! The client handle: cheap to clone, shared between threads, closed when the last clone drops.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, bounded};

use super::builder::ClientBuilder;
use super::dispatch::{Dispatcher, Outbox, Shared};
use super::error::{Error, Result};
use super::event::Queued;
use super::spawn;
use super::waiters::{Answer, Want};
use crate::engine::agent_port::AgentPort;
use crate::engine::clock::Clock;
use crate::engine::log::Log;
use crate::types::model::ChatProfile;
use crate::types::{Config, Request};

/// A running SimpleX client. Clones share one engine; methods may be called from any thread.
#[derive(Clone)]
pub struct Client {
    pub(super) inner: Arc<Inner>,
}

pub(super) struct Inner {
    requests: Mutex<Option<Sender<Request>>>,
    pub(super) events: Receiver<Queued>,
    pub(super) shared: Arc<Shared>,
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
        // Here, so `profile()` and `friends()` answer as soon as `start` returns.
        let mut dispatcher = Dispatcher::new(Arc::clone(&shared), outbox);
        started.startup.into_iter().for_each(|e| dispatcher.take(e));
        let dispatcher = dispatcher.spawn(started.events)?;
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

    /// The user's profile as the engine last reported it.
    #[must_use]
    pub fn profile(&self) -> ChatProfile {
        let p = self.inner.shared.profile.lock();
        p.unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// Stops the engine for every clone: the network closes, the store is saved and closed,
    /// waiting calls fail with `Closed` and `events()` ends once drained.
    pub fn close(&self) {
        self.inner.shutdown();
    }

    /// Sends `request` and waits for the answer to `want`.
    pub(super) fn ask(&self, want: Want, request: Request) -> Result<Answer> {
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
