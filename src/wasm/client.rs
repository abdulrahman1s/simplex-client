//! The `Client` class JS gets: start, events, save, close, and the plumbing every method uses.
use std::rc::Rc;

use serde_json::json;
use wasm_bindgen::prelude::*;

use super::clock::JsClock;
use super::convert::{error, from_js, to_js};
use super::options::Options;
use super::runtime::Core;
use super::shared::Shared;
use super::timer::Timer;
use super::waiters::Want;
use crate::engine::Clock;
use crate::store::StoreError;
use crate::types::Request;

/// A running client in this tab. Every method runs to completion on this thread; methods that
/// wait for the engine's answer return Promises, which reject with `{code, detail}`.
#[wasm_bindgen]
pub struct Client {
    pub(super) shared: Rc<Shared>,
}

fn store_error(e: &StoreError) -> JsValue {
    let code = match e {
        StoreError::WrongKey => "wrong_key",
        StoreError::Corrupt(_) => "corrupt",
        StoreError::Open(_) | StoreError::Io(_) | StoreError::Sql(_) | StoreError::Invalid(_) => {
            "store"
        }
    };
    error(code, e)
}

fn closed() -> JsValue {
    error("closed", "the client is closed")
}

#[wasm_bindgen]
impl Client {
    /// Opens the store (empty, or from `options.snapshot`) and starts the engine. Relays are
    /// dialled through `options.tunnel` as the engine needs them.
    pub fn start(options: JsValue) -> js_sys::Promise {
        match Self::open(&options) {
            Ok(client) => js_sys::Promise::resolve(&JsValue::from(client)),
            Err(e) => js_sys::Promise::reject(&e),
        }
    }

    /// Calls `handler` for each `name` event: `event` (every engine event), `message`,
    /// `request`, `ready`, `offline`, `chats`, and `change` when the store changed.
    pub fn on(&self, name: &str, handler: js_sys::Function) {
        self.shared.handlers.borrow_mut().add(name, handler);
    }

    pub fn off(&self, name: &str, handler: &js_sys::Function) {
        self.shared.handlers.borrow_mut().remove(name, handler);
    }

    /// Sends any request as the engine's serde JSON (`{t: "send", c: {…}}`); the escape hatch
    /// for everything without its own method.
    pub fn request(&self, request: JsValue) -> Result<(), JsValue> {
        let r: Request = serde_json::from_value(from_js(&request))
            .map_err(|e| error("bad_argument", format!("not a request: {e}")))?;
        self.tell(r)
    }

    /// The user's profile as the engine last reported it.
    pub fn profile(&self) -> JsValue {
        let core = self.shared.core.borrow();
        let p = core.as_ref().and_then(|c| c.sugar.profile.as_ref());
        p.map_or(JsValue::NULL, |p| to_js(&json!(p)))
    }

    /// The whole store, sealed under the key, for `Client.start({snapshot})` later.
    pub fn save(&self) -> Result<Vec<u8>, JsValue> {
        let mut core = self.shared.core.try_borrow_mut().map_err(|_| closed())?;
        let core = core.as_mut().ok_or_else(closed)?;
        core.engine
            .store
            .export_snapshot()
            .map_err(|e| store_error(&e))
    }

    /// Stops the client: sockets close, waiting calls reject with `closed`, the timer stops.
    pub fn close(&self) {
        self.shared.close();
    }
}

impl Client {
    /// `start` without the Promise, for Rust callers.
    pub fn open(options: &JsValue) -> Result<Client, JsValue> {
        let o = Options::parse(options)?;
        let shared = Shared::new();
        let core = Core::open(&o, &shared).map_err(|e| store_error(&e))?;
        *shared.core.borrow_mut() = Some(core);
        let weak = Rc::downgrade(&shared);
        let timer = Timer::new(move || {
            if let Some(shared) = weak.upgrade() {
                shared.run();
            }
        });
        // The start's events go out on the next turn, after the caller has added its handlers.
        timer.arm(0);
        *shared.timer.borrow_mut() = Some(timer);
        Ok(Client { shared })
    }

    pub(super) fn tell(&self, r: Request) -> Result<(), JsValue> {
        if self.shared.core.borrow().is_none() {
            return Err(closed());
        }
        self.shared.request(r);
        Ok(())
    }

    /// Sends `r`; the promise settles with the engine's answer to `want`, or a timeout.
    pub(super) fn ask(&self, want: Want, r: Request) -> js_sys::Promise {
        let mut parts = None;
        let promise = js_sys::Promise::new(&mut |resolve, reject| parts = Some((resolve, reject)));
        let Some(parts) = parts else {
            return js_sys::Promise::reject(&closed());
        };
        let added = self.shared.core.borrow_mut().as_mut().map(|core| {
            let deadline = JsClock.wall_ms().saturating_add(core.timeout_ms);
            core.waiters.add(want, parts, deadline);
        });
        if added.is_none() {
            return js_sys::Promise::reject(&closed());
        }
        self.shared.request(r);
        promise
    }

    pub(super) fn next_tag(&self) -> u64 {
        let mut core = self.shared.core.borrow_mut();
        core.as_mut().map_or(0, |c| {
            c.next_tag += 1;
            c.next_tag
        })
    }
}
