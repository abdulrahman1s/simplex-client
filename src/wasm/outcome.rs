//! What one run leaves for JS: handler calls and settled promises, delivered once no borrow is held.
use std::cell::RefCell;

use serde_json::Value;
use wasm_bindgen::JsValue;

use super::convert::to_js;
use super::handlers::{Handlers, call};

/// A waiting call's answer: its promise's two functions and how it ends.
pub(super) struct Settled {
    pub(super) resolve: js_sys::Function,
    pub(super) reject: js_sys::Function,
    pub(super) result: Result<Value, Value>,
}

#[derive(Default)]
pub(super) struct Outcome {
    /// Handler name and argument, in the order the events happened.
    pub(super) calls: Vec<(&'static str, Value)>,
    pub(super) settled: Vec<Settled>,
}

impl Outcome {
    pub(super) fn deliver(self, handlers: &RefCell<Handlers>) {
        for s in self.settled {
            let (f, v) = match &s.result {
                Ok(v) => (&s.resolve, v),
                Err(v) => (&s.reject, v),
            };
            // A promise's own functions do not throw.
            let _ = f.call1(&JsValue::NULL, &to_js(v));
        }
        for (name, arg) in self.calls {
            call(handlers, name, &arg);
        }
    }
}
