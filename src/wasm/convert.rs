//! Values across the JS boundary: JSON both ways, ids as numbers, failures as `{code, detail}`.
use serde_json::{Value, json};
use wasm_bindgen::JsValue;

/// A JSON value as a plain JS object; `undefined` for the one thing JSON cannot hold.
pub(super) fn to_js(v: &Value) -> JsValue {
    js_sys::JSON::parse(&v.to_string()).unwrap_or(JsValue::UNDEFINED)
}

/// A JS value as JSON; `null` for what JSON cannot hold (functions, cycles).
pub(super) fn from_js(v: &JsValue) -> Value {
    let text = js_sys::JSON::stringify(v).ok().and_then(|s| s.as_string());
    text.and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Value::Null)
}

/// Any serde value as a plain JS object.
pub(super) fn plain(v: &impl serde::Serialize) -> JsValue {
    to_js(&serde_json::to_value(v).unwrap_or(Value::Null))
}

/// What a failed call throws or rejects with.
pub(super) fn failure(code: &str, detail: impl std::fmt::Display) -> Value {
    json!({ "code": code, "detail": detail.to_string() })
}

pub(super) fn error(code: &str, detail: impl std::fmt::Display) -> JsValue {
    to_js(&failure(code, detail))
}

pub(super) fn closed() -> JsValue {
    error("closed", "the client is closed")
}

/// An engine id as a JS number; ids are counters, far below 2^53.
#[allow(clippy::cast_precision_loss, reason = "ids are small counters")]
pub(super) const fn num(id: i64) -> f64 {
    id as f64
}

/// A whole number from JS, where every number is a double.
pub(super) fn whole(n: f64) -> Option<i64> {
    #[allow(
        clippy::cast_possible_truncation,
        reason = "checked whole and in range"
    )]
    let id = (n.fract() == 0.0 && n.abs() < 9e15).then_some(n as i64);
    id
}

/// A promise already settled with `r`: what methods that only queue a request return.
pub(super) fn settled(r: Result<(), JsValue>) -> js_sys::Promise {
    match r {
        Ok(()) => js_sys::Promise::resolve(&JsValue::UNDEFINED),
        Err(e) => js_sys::Promise::reject(&e),
    }
}
