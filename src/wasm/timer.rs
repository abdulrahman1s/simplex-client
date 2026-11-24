//! The runtime's one timer: the global `setTimeout`, re-armed after every input, cleared on close.
use std::cell::RefCell;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    /// The global one, so it works in a page, a worker, Node.js and Bun alike. Node.js returns an object.
    #[wasm_bindgen(js_name = setTimeout)]
    fn set_timeout(f: &js_sys::Function, ms: f64) -> JsValue;
    #[wasm_bindgen(js_name = clearTimeout)]
    fn clear_timeout(handle: &JsValue);
}

/// One closure for the life of the client; one pending timeout at a time.
pub(super) struct Timer {
    fire: RefCell<Option<Closure<dyn FnMut()>>>,
    pending: RefCell<Option<JsValue>>,
}

impl Timer {
    pub(super) fn new(fire: impl FnMut() + 'static) -> Self {
        Self {
            fire: RefCell::new(Some(Closure::new(fire))),
            pending: RefCell::new(None),
        }
    }

    /// Replaces the pending timeout with one `ms` from now.
    pub(super) fn arm(&self, ms: u64) {
        self.clear();
        if let Some(f) = self.fire.borrow().as_ref() {
            let handle = set_timeout(f.as_ref().unchecked_ref(), ms as f64);
            *self.pending.borrow_mut() = Some(handle);
        }
    }

    fn clear(&self) {
        if let Some(handle) = self.pending.borrow_mut().take() {
            clear_timeout(&handle);
        }
    }

    /// Clears the timeout and lets the closure go; the timer never fires again.
    pub(super) fn stop(&self) {
        self.clear();
        self.fire.borrow_mut().take();
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        self.clear();
    }
}
