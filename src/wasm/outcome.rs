//! What one run leaves for JS: handler calls and settled promises, delivered once no borrow is held.
use std::collections::VecDeque;
use std::rc::Rc;

use serde_json::Value;
use wasm_bindgen::JsValue;

use super::convert::{failure, to_js};
use super::files::Parts;
use super::handlers::call;
use super::payload::Payload;
use super::shared::Shared;
use crate::engine::error::EngineError;
use crate::types::Request;

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
    /// Requests that will never run: their promises fail with `closed`.
    pub(super) fn closed(requests: VecDeque<(Request, Option<Parts>)>) -> Self {
        let settled = requests.into_iter().filter_map(|(_, parts)| {
            let (resolve, reject) = parts?;
            let result = Err(failure("closed", "the client is closed"));
            Some(Settled {
                resolve,
                reject,
                result,
            })
        });
        Self {
            calls: Vec::new(),
            settled: settled.collect(),
        }
    }

    /// The engine's verdict on a request a promise waits for: taken, or refused by a limit.
    /// Other failures still resolve it; they arrive as a `raw` `Failed` event, or for a
    /// tagged send as its `sendStatus`. A tagged send resolves with its tag.
    pub(super) fn verdict(
        &mut self,
        parts: Parts,
        done: &Result<(), EngineError>,
        tag: Option<u64>,
    ) {
        let (resolve, reject) = parts;
        let taken = tag.map_or(Payload::Nothing, |t| Payload::Json(t.into()));
        let result = match done {
            Err(e) if e.code.is_limit() => Err(failure(e.code.name(), &e.detail)),
            Ok(()) | Err(_) => Ok(taken),
        };
        self.settled.push(Settled {
            resolve,
            reject,
            result,
        });
    }

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
