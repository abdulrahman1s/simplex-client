//! Calls waiting for an answer from the engine, matched in the order they were made.
use std::collections::VecDeque;

use serde_json::{Value, json};

use super::convert::failure;
use super::outcome::Settled;
use super::sugar::message_value;
use crate::types::{Event, RequestKind};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Want {
    Invite,
    Address,
    History(u64),
    Members(i64),
    Code(i64),
}

impl Want {
    /// Two wants of one kind are answered in the order they were made.
    const fn kind(self) -> u8 {
        match self {
            Want::Invite => 0,
            Want::Address => 1,
            Want::History(_) => 2,
            Want::Members(_) => 3,
            Want::Code(_) => 4,
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

/// The want an event answers, `None` in it standing for any of that kind, and the answer.
fn answer(e: &Event) -> Option<(Want, bool, Result<Value, Value>)> {
    let ok = |want, exact, v| Some((want, exact, Ok(v)));
    match e {
        Event::Invite(link) => ok(Want::Invite, true, json!(link)),
        Event::Address(Some(link)) => ok(Want::Address, true, json!(link)),
        Event::History { tag, messages, .. } => {
            let list = messages.iter().map(message_value).collect();
            ok(Want::History(*tag), true, Value::Array(list))
        }
        Event::Members { group, members } => ok(Want::Members(*group), true, json!(members)),
        Event::Code { friend, code } => ok(Want::Code(*friend), true, json!(code)),
        Event::Failed {
            request,
            code,
            detail,
        } => {
            let want = match request {
                RequestKind::CreateInvite => Want::Invite,
                RequestKind::Address => Want::Address,
                RequestKind::History => Want::History(0),
                RequestKind::Members => Want::Members(0),
                RequestKind::Code => Want::Code(0),
                RequestKind::Start
                | RequestKind::SetProfile
                | RequestKind::Connect
                | RequestKind::Answer
                | RequestKind::Send
                | RequestKind::MarkRead
                | RequestKind::Refresh
                | RequestKind::Group
                | RequestKind::RemoveFriend
                | RequestKind::SendFile
                | RequestKind::Edit
                | RequestKind::Delete
                | RequestKind::React
                | RequestKind::Mute
                | RequestKind::Block
                | RequestKind::File
                | RequestKind::GroupLink
                | RequestKind::Call => return None,
            };
            let code = serde_json::to_value(code).unwrap_or(Value::Null);
            let why = json!({ "code": code, "detail": detail });
            Some((want, false, Err(why)))
        }
        Event::Features { .. }
        | Event::Ready(_)
        | Event::Profile(_)
        | Event::Chats { .. }
        | Event::Address(None)
        | Event::Messages(_)
        | Event::Updated(_)
        | Event::Verified { .. }
        | Event::Offline(_)
        | Event::Preview { .. }
        | Event::OpenFile { .. }
        | Event::Located { .. }
        | Event::GroupLink { .. }
        | Event::Call(_)
        | Event::Stream(_)
        | Event::Exited { .. } => None,
    }
}

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

fn settle(w: Waiter, result: Result<Value, Value>) -> Settled {
    Settled {
        resolve: w.resolve,
        reject: w.reject,
        result,
    }
}
