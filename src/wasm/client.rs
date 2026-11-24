//! The `Client` class JS gets: start, events, the escape hatch, save and close.
use std::rc::Rc;

use wasm_bindgen::prelude::*;

use super::convert::{closed, error, from_js, plain, settled};
use super::options::Options;
use super::runtime::Core;
use super::shared::Shared;
use super::timer::Timer;
use crate::store::StoreError;

#[wasm_bindgen(typescript_custom_section)]
const EVENTS: &str = r#"
/** What `on`, `once` and `off` take: each event and what its handler gets. */
export type ClientEvent =
  | "ready" | "message" | "messageUpdate" | "request"
  | "friendAdd" | "friendRemove" | "groupAdd" | "groupRemove" | "groupInvite"
  | "memberAdd" | "offline" | "online" | "change" | "raw" | "fileProgress";
"#;

/// A running client in this tab or process. Methods return Promises that reject with
/// `{code, detail}`; the objects events carry (`Message`, `Friend`, …) act through it.
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

#[wasm_bindgen]
impl Client {
    /// Opens the store (empty, or from `options.snapshot`) and starts the engine. Relays are
    /// dialled through `options.tunnel`, or over TCP on Node.js and Bun.
    #[wasm_bindgen(unchecked_return_type = "Promise<Client>")]
    pub fn start(options: JsValue) -> js_sys::Promise {
        match Self::open(&options) {
            Ok(client) => js_sys::Promise::resolve(&JsValue::from(client)),
            Err(e) => js_sys::Promise::reject(&e),
        }
    }

    /// Calls `handler` for each `name` event: `ready` (the profile), `message` and
    /// `messageUpdate` (a `Message`), `request` (a `FriendRequest`), `friendAdd` and
    /// `friendRemove` (a `Friend`), `groupAdd` and `groupRemove` (a `Group`), `groupInvite`
    /// (a `GroupInvite`), `memberAdd` (a `Member` who connected with you: someone new, or each
    /// one already there when you join), `offline`, `online`, `change` (the store
    /// changed: save it) and `raw` (every engine event, as JSON).
    pub fn on(
        &self,
        #[wasm_bindgen(unchecked_param_type = "ClientEvent")] name: &str,
        handler: js_sys::Function,
    ) {
        self.shared.handlers.borrow_mut().add(name, handler, false);
    }

    pub fn off(
        &self,
        #[wasm_bindgen(unchecked_param_type = "ClientEvent")] name: &str,
        handler: &js_sys::Function,
    ) {
        self.shared.handlers.borrow_mut().remove(name, handler);
    }

    /// The next `name` event: `handler`, if given, runs for it once, and the promise
    /// resolves with what it carries.
    #[wasm_bindgen(unchecked_return_type = "Promise<any>")]
    pub fn once(
        &self,
        #[wasm_bindgen(unchecked_param_type = "ClientEvent")] name: &str,
        handler: Option<js_sys::Function>,
    ) -> js_sys::Promise {
        let mut handlers = self.shared.handlers.borrow_mut();
        if let Some(h) = handler {
            handlers.add(name, h, true);
        }
        js_sys::Promise::new(&mut |resolve, _| handlers.add(name, resolve, true))
    }

    /// Sends any request as the engine's serde JSON (`{t: "send", c: {…}}`); the escape hatch
    /// for everything without its own method. What it causes arrives as events.
    #[wasm_bindgen(unchecked_return_type = "Promise<void>")]
    pub fn request(&self, request: JsValue) -> js_sys::Promise {
        let r = serde_json::from_value(from_js(&request))
            .map_err(|e| error("bad_argument", format!("not a request: {e}")));
        settled(r.and_then(|r| self.shared.tell(r)))
    }

    /// The user's profile as the engine last reported it.
    #[wasm_bindgen(getter)]
    pub fn user(&self) -> JsValue {
        self.shared.profile().map_or(JsValue::NULL, |p| plain(&p))
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
}
