//! Direct TCP to a relay on Node through `node:net`: a few lines of JS behind the pool's pipe; TLS stays in wasm.
use std::rc::Weak;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

use super::shared::Shared;
use crate::smp::{Dialer, Pipe, PipeId, PipeInput, SmpError};

/// `connect(host, port, on)`: calls `on("open")`, `on("data", bytes)` and one `on("close",
/// why)`; returns `{write, close}`. It is made with `Function`, not imported, so a browser
/// build that never dials TCP never loads `node:net`.
const SHIM: &str = r#"
const net = globalThis.process?.getBuiltinModule?.("node:net");
if (!net) throw new Error("direct TCP needs Node.js 22 or newer");
const s = net.connect({ host, port });
s.setNoDelay(true);
let done = false;
const end = (why) => { if (!done) { done = true; on("close", why); } };
s.on("connect", () => on("open"));
s.on("data", (b) => on("data", new Uint8Array(b.buffer, b.byteOffset, b.byteLength)));
s.on("error", (e) => end(String(e && e.message)));
s.on("close", () => end("closed"));
return {
  write: (bytes) => { s.write(Buffer.from(bytes)); },
  close: () => { done = true; s.removeAllListeners(); s.on("error", () => {}); s.destroy(); },
};
"#;

fn io(e: &JsValue) -> SmpError {
    let msg = js_sys::Reflect::get(e, &"message".into()).ok();
    let msg = msg.and_then(|m| m.as_string()).or_else(|| e.as_string());
    SmpError::Io(msg.unwrap_or_else(|| "TCP failed".into()))
}

/// Dials each relay straight over TCP; only on Node.
pub(super) struct TcpDialer {
    pub(super) shared: Weak<Shared>,
}

impl Dialer for TcpDialer {
    fn dial(&mut self, id: PipeId, host: &str, port: u16) -> Result<Box<dyn Pipe>, SmpError> {
        let connect = js_sys::Function::new_with_args("host, port, on", SHIM);
        let shared = Weak::clone(&self.shared);
        let on = Closure::<dyn FnMut(String, JsValue)>::new(move |kind: String, data: JsValue| {
            let input = match kind.as_str() {
                "open" => PipeInput::Open,
                "data" => match data.dyn_ref::<js_sys::Uint8Array>() {
                    Some(bytes) => PipeInput::Binary(bytes.to_vec()),
                    None => return,
                },
                _ => PipeInput::Closed {
                    code: 0,
                    reason: data.as_string().unwrap_or_default(),
                },
            };
            if let Some(shared) = shared.upgrade() {
                shared.post(id, input);
            }
        });
        let socket = connect
            .call3(&JsValue::NULL, &host.into(), &port.into(), on.as_ref())
            .map_err(|e| io(&e))?;
        Ok(Box::new(TcpPipe { socket, _on: on }))
    }

    fn tunnelled(&self) -> bool {
        false
    }
}

/// The socket's `{write, close}` and the callback it reports through; `close` unhooks the
/// socket before the callback is dropped.
struct TcpPipe {
    socket: JsValue,
    _on: Closure<dyn FnMut(String, JsValue)>,
}

impl TcpPipe {
    fn call(&self, method: &str, arg: &JsValue) -> Result<(), SmpError> {
        let f = js_sys::Reflect::get(&self.socket, &method.into()).map_err(|e| io(&e))?;
        let f: js_sys::Function = f.dyn_into().map_err(|e| io(&e))?;
        f.call1(&JsValue::NULL, arg).map(drop).map_err(|e| io(&e))
    }
}

impl Pipe for TcpPipe {
    fn send_text(&mut self, _: &str) -> Result<(), SmpError> {
        Err(SmpError::Unexpected("text on a TCP pipe"))
    }

    fn send_binary(&mut self, bytes: &[u8]) -> Result<(), SmpError> {
        self.call("write", &js_sys::Uint8Array::from(bytes).into())
    }

    fn close(&mut self) {
        // A socket that is gone already has nothing left to close.
        let _ = self.call("close", &JsValue::UNDEFINED);
    }
}

impl Drop for TcpPipe {
    fn drop(&mut self) {
        self.close();
    }
}
