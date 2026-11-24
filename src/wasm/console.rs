//! The engine's log on the JS console, one `console.log` per line.
use std::io::{Result, Write};

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console, js_name = log)]
    fn console_log(line: &str);
    #[wasm_bindgen(js_namespace = console, js_name = error)]
    pub(super) fn console_error(what: &str, error: &JsValue);
}

/// Bytes wait here until a newline; `Log::line` writes one line and flushes.
#[derive(Default)]
pub(super) struct Console(Vec<u8>);

impl Write for Console {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        self.0.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<()> {
        let text = String::from_utf8_lossy(&self.0);
        for line in text.lines() {
            console_log(line);
        }
        self.0.clear();
        Ok(())
    }
}
