//! `setTimer` on a friend or a group: seconds new messages last, or `null` for off.

use std::rc::Rc;

use super::super::convert::{error, settled, whole};
use super::super::shared::Shared;
use crate::types::Request;
use crate::types::model::ChatRef;

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
