//! The dispatcher thread: answers go to the calls waiting for them, the rest to `events()`.

use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};

use crossbeam_channel::{Receiver, Sender, TrySendError};

use super::waiters::Waiters;
use crate::engine::log::Log;
use crate::types::Event;
use crate::types::model::ChatProfile;

/// What the dispatcher shares with every `Client` clone. It never holds the client itself,
/// so dropping the last clone still shuts down.
#[derive(Default)]
pub(super) struct Shared {
    pub(super) waiters: Waiters,
    pub(super) profile: Mutex<ChatProfile>,
}

impl Shared {
    pub(super) fn note(&self, event: &Event) {
        if let Event::Ready(p) | Event::Profile(p) = event {
            p.clone_into(&mut self.profile.lock().unwrap_or_else(PoisonError::into_inner));
        }
    }
}

/// The user's side of the events: a bounded queue that keeps the newest events. When it is
/// full the oldest is dropped, so a program that never reads `events()` still gets its answers.
pub(super) struct Outbox {
    pub(super) tx: Sender<Event>,
    /// Kept to drop the oldest event when the queue is full.
    pub(super) rx: Receiver<Event>,
    pub(super) log: Log,
}

impl Outbox {
    /// Queues `event`, dropping the oldest ones while the queue is full.
    pub(super) fn deliver(&self, mut event: Event) -> usize {
        let mut dropped = 0;
        loop {
            match self.tx.try_send(event) {
                Ok(()) | Err(TrySendError::Disconnected(_)) => return dropped,
                Err(TrySendError::Full(back)) => {
                    event = back;
                    if self.rx.try_recv().is_ok() {
                        dropped += 1;
                    }
                }
            }
        }
    }
}

pub(super) fn spawn(
    engine: Receiver<Event>,
    startup: Vec<Event>,
    shared: Arc<Shared>,
    outbox: Outbox,
) -> std::io::Result<JoinHandle<()>> {
    thread::Builder::new()
        .name("simplex-dispatch".into())
        .spawn(move || {
            let mut dropping = false;
            let mut put = |e: Event| {
                let dropped = outbox.deliver(e);
                if dropped > 0 && !dropping {
                    outbox
                        .log
                        .line(format_args!("events() is not read: dropping the oldest"));
                }
                dropping = dropped > 0;
            };
            for e in startup {
                shared.note(&e);
                put(e);
            }
            for e in &engine {
                shared.note(&e);
                if let Some(e) = shared.waiters.route(e) {
                    put(e);
                }
            }
            shared.waiters.close();
        })
}
