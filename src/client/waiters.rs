//! Calls waiting for their answer. One call of each kind waits at a time, so a failure, which
//! names only the request's kind, always belongs to the caller waiting on that kind.

use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use crossbeam_channel::{RecvTimeoutError, Sender, bounded};

use super::answers::answers;
use super::error::{Error, Result};
use crate::types::Event;
use crate::types::model::ChatGroup;

/// What a waiting call gets: the engine's event, or a group the chat list shows as new.
#[derive(Debug)]
pub(super) enum Answer {
    Event(Event),
    Group(ChatGroup),
}

/// What a waiting call takes as its answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Want {
    Invite,
    Address,
    History(u64),
    Members(i64),
    Code(i64),
    /// The next group the user made.
    NewGroup,
}

const KINDS: usize = 6;

impl Want {
    pub(super) const fn slot(self) -> usize {
        match self {
            Self::Invite => 0,
            Self::Address => 1,
            Self::History(_) => 2,
            Self::Members(_) => 3,
            Self::Code(_) => 4,
            Self::NewGroup => 5,
        }
    }
}

type Slot = Option<(Want, Sender<Result<Answer>>)>;

/// At most one entry per kind; an entry leaves when answered, timed out or closed.
#[derive(Default)]
pub(super) struct Waiters {
    turns: [Mutex<()>; KINDS],
    slots: Mutex<[Slot; KINDS]>,
}

impl Waiters {
    /// Registers `want`, runs `send`, and waits up to `timeout` for the answer.
    pub(super) fn wait(
        &self,
        want: Want,
        timeout: Duration,
        send: impl FnOnce() -> Result<()>,
    ) -> Result<Answer> {
        let _turn = self.turns[want.slot()]
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let (tx, rx) = bounded(1);
        self.slots()[want.slot()] = Some((want, tx));
        if let Err(e) = send() {
            self.slots()[want.slot()] = None;
            return Err(e);
        }
        match rx.recv_timeout(timeout) {
            Ok(answer) => answer,
            Err(RecvTimeoutError::Timeout) => {
                self.slots()[want.slot()] = None;
                // The answer may have come between the timeout and taking the slot.
                rx.try_recv().unwrap_or(Err(Error::Timeout))
            }
            Err(RecvTimeoutError::Disconnected) => Err(Error::Closed),
        }
    }

    /// Hands `event` to the call waiting for it; an event nobody waits for comes back.
    pub(super) fn route(&self, event: Event) -> Option<Event> {
        let Some((slot, exact)) = answers(&event) else {
            return Some(event);
        };
        let mut slots = self.slots();
        let taken = match (&slots[slot], exact) {
            (Some((want, _)), Some(exact)) if *want != exact => None,
            (Some(_), _) => slots[slot].take(),
            (None, _) => None,
        };
        drop(slots);
        let Some((_, tx)) = taken else {
            return Some(event);
        };
        let answer = match event {
            Event::Failed {
                request,
                code,
                detail,
            } => Err(Error::Failed {
                request,
                code,
                detail,
            }),
            other => Ok(Answer::Event(other)),
        };
        // The caller may have just timed out; its answer then has nowhere to go.
        let _ = tx.send(answer);
        None
    }

    /// Hands a group the user owns, new in the chat list, to the `create_group` waiting.
    pub(super) fn created(&self, group: &ChatGroup) {
        let taken = self.slots()[Want::NewGroup.slot()].take();
        if let Some((_, tx)) = taken {
            // The caller may have just timed out; the group then shows in events only.
            let _ = tx.send(Ok(Answer::Group(group.clone())));
        }
    }

    /// Every waiting call ends with `Closed`.
    pub(super) fn close(&self) {
        *self.slots() = Default::default();
    }

    fn slots(&self) -> std::sync::MutexGuard<'_, [Slot; KINDS]> {
        self.slots.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
