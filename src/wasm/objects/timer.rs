//! `setTimer` on a friend or a group, and `setPrefs` on the client or a friend.

use std::rc::Rc;

use super::super::convert::{error, from_js, settled, whole};
use super::super::shared::Shared;
use crate::types::Request;
use crate::types::model::{ChatPrefs, ChatRef};

/// Queues `SetTimer`; anything but a whole number of seconds or `null` is refused here.
pub(super) fn set_timer(
    shared: &Rc<Shared>,
    chat: ChatRef,
    seconds: Option<f64>,
) -> js_sys::Promise {
    let ttl = match seconds.map(whole) {
        None => None,
        Some(Some(s)) => Some(s),
        Some(None) => {
            let e = error("bad_argument", "the timer is a whole number of seconds");
            return settled(Err(e));
        }
    };
    shared.queue(Request::SetTimer { chat, ttl })
}

/// Queues `SetPrefs`: `{ voice: "no", calls: "always", … }`, each `"always"`, `"yes"` or `"no"`.
pub(in crate::wasm) fn set_prefs(
    shared: &Rc<Shared>,
    friend: Option<i64>,
    prefs: &wasm_bindgen::JsValue,
) -> js_sys::Promise {
    match serde_json::from_value::<ChatPrefs>(from_js(prefs)) {
        Ok(prefs) => shared.queue(Request::SetPrefs { friend, prefs }),
        Err(e) => settled(Err(error("bad_argument", format!("not preferences: {e}")))),
    }
}
