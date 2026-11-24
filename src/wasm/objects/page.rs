//! A page of history: `{limit, before}` from JS, asked of the engine.
use std::rc::Rc;

use wasm_bindgen::JsValue;
use wasm_bindgen::prelude::*;

use super::super::convert::{error, whole};
use super::super::shared::Shared;
use super::super::waiters::Want;
use crate::types::Request;
use crate::types::model::ChatRef;

#[wasm_bindgen(typescript_custom_section)]
const PAGE: &str = r#"
/** Which messages `history` returns: the newest `limit` (50) before message `before`. */
export interface HistoryOptions { limit?: number; before?: number }
"#;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(typescript_type = "HistoryOptions")]
    pub type HistoryOptions;
}

/// History pages are this long unless the caller says otherwise.
const LIMIT: u32 = 50;

fn number(o: &JsValue, key: &str) -> Option<f64> {
    js_sys::Reflect::get(o, &key.into()).ok()?.as_f64()
}

/// Up to `limit` messages of `chat` before `before` (or the newest), oldest first.
pub(super) fn history(
    shared: &Rc<Shared>,
    chat: ChatRef,
    options: Option<HistoryOptions>,
) -> js_sys::Promise {
    let options: JsValue = options.map_or(JsValue::UNDEFINED, Into::into);
    let options = &options;
    let limit = number(options, "limit").and_then(whole);
    let count = limit.map_or(Some(LIMIT), |n| u32::try_from(n).ok());
    let before = number(options, "before").map(whole);
    let (Some(count), Some(before)) = (count, before.map_or(Some(None), |b| b.map(Some))) else {
        let e = error("bad_argument", "limit and before are whole numbers");
        return js_sys::Promise::reject(&e);
    };
    let tag = shared.next_tag();
    let request = Request::History {
        chat,
        before,
        count,
        tag,
    };
    shared.ask(Want::History(tag), request)
}
