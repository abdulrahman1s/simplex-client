//! The dispatcher thread: answers go to the calls waiting for them, the rest to `events()`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};

use crossbeam_channel::{Receiver, Sender, TrySendError};

use super::event::Queued;
use super::waiters::Waiters;
use crate::engine::log::Log;
use crate::roster::{Note, Roster, lift};
use crate::types::Event;
use crate::types::model::ChatProfile;

/// What the dispatcher shares with every `Client` clone. It never holds the client itself,
/// so dropping the last clone still shuts down.
#[derive(Default)]
pub(super) struct Shared {
    pub(super) waiters: Waiters,
    pub(super) profile: Mutex<ChatProfile>,
    pub(super) roster: Mutex<Roster>,
    /// `Client::refresh` asked for the whole list, which a client hearing deltas then hears.
    pub(super) refresh: AtomicBool,
}

impl Shared {
    /// Keeps the profile and the chat list; what `event` means to the user.
    pub(super) fn note(&self, event: &Event) -> Vec<Note> {
        if let Event::Ready(p) | Event::Profile(p) = event {
            p.clone_into(&mut self.profile.lock().unwrap_or_else(PoisonError::into_inner));
        }
        let notes = lift(
            event,
            &mut self.roster.lock().unwrap_or_else(PoisonError::into_inner),
        );
        for n in &notes {
            if let Note::GroupAdd(g) = n
                && g.owner
            {
                self.waiters.created(g);
            }
        }
        notes
    }
}

/// Engine events that have no variant of their own in `client::Event`.
const fn raw(e: &Event) -> bool {
    match e {
        Event::Ready(_)
        | Event::Messages(_)
        | Event::Updated(_)
        | Event::Offline(_)
        | Event::Cleared(_)
        | Event::Data { .. }
        | Event::SendStatus { .. }
        | Event::ChatsChanged { .. } => false,
        Event::Features { .. }
        | Event::Profile(_)
        | Event::Chats { .. }
        | Event::Invite(_)
        | Event::Address(_)
        | Event::History { .. }
        | Event::Code { .. }
        | Event::Verified { .. }
        | Event::Members { .. }
        | Event::Preview { .. }
        | Event::OpenFile { .. }
        | Event::Located { .. }
        | Event::GroupLink { .. }
        | Event::Call(_)
        | Event::Stream(_)
        | Event::Failed { .. }
        | Event::Exited { .. } => true,
    }
}

/// The user's side of the events: a bounded queue that keeps the newest events. When it is
/// full the oldest is dropped, so a program that never reads `events()` still gets its answers.
pub(super) struct Outbox {
    pub(super) tx: Sender<Queued>,
    /// Kept to drop the oldest event when the queue is full.
    pub(super) rx: Receiver<Queued>,
    pub(super) log: Log,
}

impl Outbox {
    /// Queues `item`, dropping the oldest ones while the queue is full.
    pub(super) fn deliver(&self, mut item: Queued) -> usize {
        let mut dropped = 0;
        loop {
            match self.tx.try_send(item) {
                Ok(()) | Err(TrySendError::Disconnected(_)) => return dropped,
                Err(TrySendError::Full(back)) => {
                    item = back;
                    if self.rx.try_recv().is_ok() {
                        dropped += 1;
                    }
                }
            }
        }
    }
}

/// Turns engine events into answers and queued events.
pub(super) struct Dispatcher {
    pub(super) shared: Arc<Shared>,
    pub(super) outbox: Outbox,
    dropping: bool,
}

impl Dispatcher {
    pub(super) const fn new(shared: Arc<Shared>, outbox: Outbox) -> Self {
        Self {
            shared,
            outbox,
            dropping: false,
        }
    }

    pub(super) fn take(&mut self, e: Event) {
        let notes = self.shared.note(&e);
        if let Some(e) = self.shared.waiters.route(e)
            && raw(&e)
            && self.wants_list(&e)
        {
            self.put(Queued::Raw(e));
        }
        notes
            .into_iter()
            .for_each(|n| self.put(Queued::Note(Box::new(n))));
    }

    /// A chat list goes to `events()` unless the client hears deltas and did not ask.
    fn wants_list(&self, e: &Event) -> bool {
        let Event::Chats { .. } = e else {
            return true;
        };
        let deltas = self.shared.roster.lock().is_ok_and(|r| r.deltas);
        !deltas || self.shared.refresh.swap(false, Ordering::Relaxed)
    }

    fn put(&mut self, q: Queued) {
        let dropped = self.outbox.deliver(q);
        if dropped > 0 && !self.dropping {
            let log = &self.outbox.log;
            log.line(format_args!("events() is not read: dropping the oldest"));
        }
        self.dropping = dropped > 0;
    }

    pub(super) fn spawn(mut self, engine: Receiver<Event>) -> std::io::Result<JoinHandle<()>> {
        thread::Builder::new()
            .name("simplex-dispatch".into())
            .spawn(move || {
                for e in &engine {
                    self.take(e);
                }
                self.shared.waiters.close();
            })
    }
}
