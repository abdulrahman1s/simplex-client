//! What one run leaves for JS: handler calls and settled promises, delivered once no borrow is held.
use std::rc::Rc;

use serde_json::Value;
use wasm_bindgen::JsValue;

use super::convert::to_js;
use super::handlers::call;
use super::payload::Payload;
use super::shared::Shared;

/// A waiting call's answer: its promise's two functions and how it ends.
pub(super) struct Settled {
    pub(super) resolve: js_sys::Function,
    pub(super) reject: js_sys::Function,
    pub(super) result: Result<Payload, Value>,
}

#[derive(Default)]
pub(super) struct Outcome {
    /// Handler name and argument, in the order the events happened.
    pub(super) calls: Vec<(&'static str, Payload)>,
    pub(super) settled: Vec<Settled>,
}

impl Outcome {
    pub(super) fn deliver(self, shared: &Rc<Shared>) {
        for s in self.settled {
            let (f, v) = match s.result {
                Ok(v) => (&s.resolve, v.into_js(shared)),
                Err(v) => (&s.reject, to_js(&v)),
            };
            // A promise's own functions do not throw.
            let _ = f.call1(&JsValue::NULL, &v);
        }
        for (name, arg) in self.calls {
            let list = shared.handlers.borrow_mut().take(name);
            if !list.is_empty() {
                call(&list, name, &arg.into_js(shared));
            }
        }
    }
}
