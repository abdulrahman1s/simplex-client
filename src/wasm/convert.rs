//! Values across the JS boundary as JSON: events out, chat refs and messages in, failures as `{code, detail}`.
use serde_json::{Value, json};
use wasm_bindgen::JsValue;

use crate::types::model::ChatRef;

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

/// What a failed call throws or rejects with.
pub(super) fn failure(code: &str, detail: impl std::fmt::Display) -> Value {
    json!({ "code": code, "detail": detail.to_string() })
}

pub(super) fn error(code: &str, detail: impl std::fmt::Display) -> JsValue {
    to_js(&failure(code, detail))
}

/// A whole number from JS, where every number is a double.
pub(super) fn whole(v: &Value) -> Option<i64> {
    match v.as_i64() {
        Some(n) => Some(n),
        None => v.as_f64().filter(|f| f.fract() == 0.0).map(|f| f as i64),
    }
}

pub(super) fn id(n: f64) -> Result<i64, JsValue> {
    whole(&json!(n)).ok_or_else(|| error("bad_argument", "an id must be a whole number"))
}

/// `{friend: 1}`, `{group: 2}`, or the serde form `{t: "direct", c: 1}` an event carries.
pub(super) fn chat_ref(v: &JsValue) -> Result<ChatRef, JsValue> {
    let v = from_js(v);
    let pick = |k: &str| v.get(k).and_then(whole);
    let chat = match (pick("friend").or(pick("direct")), pick("group")) {
        (Some(f), None) => Some(ChatRef::Direct(f)),
        (None, Some(g)) => Some(ChatRef::Group(g)),
        _ => serde_json::from_value(v.clone()).ok(),
    };
    chat.ok_or_else(|| error("bad_argument", "a chat is {friend: id} or {group: id}"))
}

/// The chat and id of a message object from a `message` event or `history`.
pub(super) fn message_ref(v: &JsValue) -> Result<(ChatRef, i64), JsValue> {
    let chat = js_sys::Reflect::get(v, &"chat".into()).unwrap_or(JsValue::UNDEFINED);
    let item = js_sys::Reflect::get(v, &"id".into()).unwrap_or(JsValue::UNDEFINED);
    let item = item
        .as_f64()
        .ok_or_else(|| error("bad_argument", "a message has an id"))?;
    Ok((chat_ref(&chat)?, id(item)?))
}
