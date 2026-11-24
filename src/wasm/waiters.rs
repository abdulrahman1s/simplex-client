//! Calls waiting for an answer from the engine, matched in the order they were made.
use std::collections::VecDeque;

use serde_json::Value;

use super::answers::answer;
use super::convert::failure;
use super::outcome::Settled;
use super::payload::Payload;
use crate::types::Event;
use crate::types::model::ChatGroup;

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

impl Want {
    /// Two wants of one kind are answered in the order they were made.
    pub(super) const fn kind(self) -> u8 {
        match self {
            Want::Invite => 0,
            Want::Address => 1,
            Want::History(_) => 2,
            Want::Members(_) => 3,
            Want::Code(_) => 4,
            Want::NewGroup => 5,
        }
    }
}

struct Waiter {
    want: Want,
    resolve: js_sys::Function,
    reject: js_sys::Function,
    /// Wall ms after which the call fails with `timeout`.
    deadline: i64,
}

/// Eviction: a waiter leaves with its answer, at its deadline, or when the client closes.
#[derive(Default)]
pub(super) struct Waiters(VecDeque<Waiter>);

impl Waiters {
    pub(super) fn add(
        &mut self,
        want: Want,
        (resolve, reject): (js_sys::Function, js_sys::Function),
        deadline: i64,
    ) {
        self.0.push_back(Waiter {
            want,
            resolve,
            reject,
            deadline,
        });
    }

    /// The call `e` answers, settled.
    pub(super) fn answer(&mut self, e: &Event) -> Option<Settled> {
        let (want, exact, result) = answer(e)?;
        let at = self
            .0
            .iter()
            .position(|w| w.want.kind() == want.kind() && (!exact || w.want == want))?;
        self.0.remove(at).map(|w| settle(w, result))
    }

    /// A group the user owns, new in the chat list, answers the oldest `createGroup`.
    pub(super) fn created(&mut self, g: &ChatGroup) -> Option<Settled> {
        let at = self.0.iter().position(|w| w.want == Want::NewGroup)?;
        let group = Payload::Group(Box::new(g.clone()));
        self.0.remove(at).map(|w| settle(w, Ok(group)))
    }

    pub(super) fn expire(&mut self, now: i64) -> Vec<Settled> {
        let (gone, kept) = std::mem::take(&mut self.0)
            .into_iter()
            .partition(|w| w.deadline <= now);
        self.0 = kept;
        let late = || Err(failure("timeout", "no answer in time"));
        gone.into_iter()
            .map(|w: Waiter| settle(w, late()))
            .collect()
    }

    pub(super) fn close(&mut self) -> Vec<Settled> {
        let closed = || Err(failure("closed", "the client is closed"));
        self.0.drain(..).map(|w| settle(w, closed())).collect()
    }

    pub(super) fn next_deadline(&self) -> Option<i64> {
        self.0.iter().map(|w| w.deadline).min()
    }
}

fn settle(w: Waiter, result: Result<Payload, Value>) -> Settled {
    Settled {
        resolve: w.resolve,
        reject: w.reject,
        result,
    }
}
