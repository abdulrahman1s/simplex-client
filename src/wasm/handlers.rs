//! The JS functions given to `client.on` and `client.once`, by event name.
use std::collections::HashMap;

use wasm_bindgen::JsValue;

use super::console::console_error;

/// Bounded by the program's own `on` calls; `off` removes one, and a `once` goes when called.
#[derive(Default)]
pub(super) struct Handlers(HashMap<String, Vec<(js_sys::Function, bool)>>);

impl Handlers {
    pub(super) fn add(&mut self, name: &str, f: js_sys::Function, once: bool) {
        self.0.entry(name.to_owned()).or_default().push((f, once));
    }

    pub(super) fn remove(&mut self, name: &str, f: &js_sys::Function) {
        if let Some(list) = self.0.get_mut(name) {
            list.retain(|(g, _)| g != f);
        }
    }

    /// The functions to call for `name`, the `once` ones taken out.
    pub(super) fn take(&mut self, name: &str) -> Vec<js_sys::Function> {
        let Some(list) = self.0.get_mut(name) else {
            return Vec::new();
        };
        let all = list.iter().map(|(f, _)| f.clone()).collect();
        list.retain(|(_, once)| !once);
        all
    }
}

/// Calls each of `list` with `arg`; one that throws is logged and the rest still run.
pub(super) fn call(list: &[js_sys::Function], name: &str, arg: &JsValue) {
    for f in list {
        if let Err(e) = f.call1(&JsValue::NULL, arg) {
            console_error(&format!("simplex-client: a \"{name}\" handler threw"), &e);
        }
    }
}
