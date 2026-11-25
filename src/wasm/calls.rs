//! How the client and its objects reach the engine: queue a request, or wait for its answer.
use std::rc::Rc;

use wasm_bindgen::JsValue;

use super::clock::JsClock;
use super::convert::{closed, settled};
use super::shared::Shared;
use super::waiters::Want;
use crate::engine::Clock;
use crate::types::Request;
use crate::types::model::{ChatGroup, ChatProfile, Friend};

impl Shared {
    /// Queues `r`; fails only when the client is closed.
    pub(super) fn tell(self: &Rc<Self>, r: Request) -> Result<(), JsValue> {
        if self.core.borrow().is_none() {
            return Err(closed());
        }
        self.request(r);
        Ok(())
    }

    /// `tell` as a promise, for methods that answer nothing but keep every method awaitable:
    /// it resolves once the engine has taken the request, and rejects with the limit's code
    /// (`reaction_not_allowed`, `text_too_long`, …) when it breaks one.
    pub(super) fn queue(self: &Rc<Self>, r: Request) -> js_sys::Promise {
        if self.core.borrow().is_none() {
            return settled(Err(closed()));
        }
        let mut parts = None;
        let promise = js_sys::Promise::new(&mut |resolve, reject| parts = Some((resolve, reject)));
        self.request_with(r, parts);
        promise
    }

    /// `queue` for a send: it goes as `Request::Tagged`, and the promise resolves with the
    /// tag its `sendStatus` events carry.
    pub(super) fn queue_send(self: &Rc<Self>, r: Request) -> js_sys::Promise {
        let tag = self.next_tag();
        self.queue(Request::Tagged {
            tag,
            request: Box::new(r),
        })
    }

    /// Sends `r`; the promise settles with the engine's answer to `want`, or a timeout.
    pub(super) fn ask(self: &Rc<Self>, want: Want, r: Request) -> js_sys::Promise {
        let mut parts = None;
        let promise = js_sys::Promise::new(&mut |resolve, reject| parts = Some((resolve, reject)));
        let Some(parts) = parts else {
            return js_sys::Promise::reject(&closed());
        };
        let added = self.core.borrow_mut().as_mut().map(|core| {
            let deadline = JsClock.wall_ms().saturating_add(core.timeout_ms);
            core.waiters.add(want, parts, deadline);
        });
        if added.is_none() {
            return js_sys::Promise::reject(&closed());
        }
        self.request(r);
        promise
    }

    pub(super) fn next_tag(&self) -> u64 {
        let mut core = self.core.borrow_mut();
        core.as_mut().map_or(0, |c| {
            c.next_tag += 1;
            c.next_tag
        })
    }

    pub(super) fn profile(&self) -> Option<ChatProfile> {
        self.core.borrow().as_ref().and_then(|c| c.profile.clone())
    }

    /// The friend `id` from the last chat list, or one known only by its id.
    pub(super) fn friend(&self, id: i64) -> Friend {
        let core = self.core.borrow();
        let known = core
            .as_ref()
            .and_then(|c| c.roster.friends.get(&id).cloned());
        known.unwrap_or_else(|| Friend {
            id,
            ..Friend::default()
        })
    }

    pub(super) fn group(&self, id: i64) -> ChatGroup {
        let core = self.core.borrow();
        let known = core
            .as_ref()
            .and_then(|c| c.roster.groups.get(&id).cloned());
        known.unwrap_or_else(|| ChatGroup {
            id,
            ..ChatGroup::default()
        })
    }
}
