//! The JS functions given to `client.on`, by event name, and what a run hands them.
use std::cell::RefCell;
use std::collections::HashMap;

use serde_json::Value;
use wasm_bindgen::JsValue;

use super::console::console_error;
use super::convert::to_js;

/// Bounded by the program's own `on` calls; `off` removes one.
#[derive(Default)]
pub(super) struct Handlers(HashMap<String, Vec<js_sys::Function>>);

impl Handlers {
    pub(super) fn add(&mut self, name: &str, f: js_sys::Function) {
        self.0.entry(name.to_owned()).or_default().push(f);
    }

    pub(super) fn remove(&mut self, name: &str, f: &js_sys::Function) {
        if let Some(list) = self.0.get_mut(name) {
            list.retain(|g| g != f);
        }
    }
}

/// Calls every handler of `name` with `arg`. The list is copied out first, so a handler may
/// add or remove handlers; one that throws is logged and the rest still run.
pub(super) fn call(handlers: &RefCell<Handlers>, name: &str, arg: &Value) {
    let Some(list) = handlers.borrow().0.get(name).cloned() else {
        return;
    };
    let arg = to_js(arg);
    for f in list {
        if let Err(e) = f.call1(&JsValue::NULL, &arg) {
            console_error(&format!("simplex-client: a \"{name}\" handler threw"), &e);
        }
    }
}
