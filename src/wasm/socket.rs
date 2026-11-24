//! The tunnel's WebSocket as the pool's pipe: what it reports goes to the runtime's inbox.
use std::rc::Weak;

use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use web_sys::{BinaryType, CloseEvent, MessageEvent, WebSocket};

use super::shared::Shared;
use crate::smp::{Dialer, Pipe, PipeId, PipeInput, SmpError};

fn io(e: &JsValue) -> SmpError {
    SmpError::Io(e.as_string().unwrap_or_else(|| "WebSocket failed".into()))
}

/// Dials `url` (`wss://…/v1`) for every relay.
pub(super) struct WsDialer {
    pub(super) url: String,
    pub(super) shared: Weak<Shared>,
}

impl Dialer for WsDialer {
    fn dial(&mut self, id: PipeId) -> Result<Box<dyn Pipe>, SmpError> {
        Ok(Box::new(WsPipe::open(&self.url, id, &self.shared)?))
    }
}

/// The socket and its handlers. The closures live as long as the socket is ours; they are
/// unhooked before they drop, so a late event finds no handler instead of a freed closure.
struct WsPipe {
    ws: WebSocket,
    _open: Closure<dyn FnMut()>,
    _message: Closure<dyn FnMut(MessageEvent)>,
    _close: Closure<dyn FnMut(CloseEvent)>,
}

/// A handler that hands `what` the event and posts its input under `id`.
fn post<E: wasm_bindgen::convert::FromWasmAbi + 'static>(
    shared: &Weak<Shared>,
    id: PipeId,
    what: impl Fn(E) -> Option<PipeInput> + 'static,
) -> Closure<dyn FnMut(E)> {
    let shared = Weak::clone(shared);
    Closure::new(move |e: E| {
        if let (Some(input), Some(shared)) = (what(e), shared.upgrade()) {
            shared.post(id, input);
        }
    })
}

impl WsPipe {
    fn open(url: &str, id: PipeId, shared: &Weak<Shared>) -> Result<Self, SmpError> {
        let ws = WebSocket::new(url).map_err(|e| io(&e))?;
        ws.set_binary_type(BinaryType::Arraybuffer);
        let open = {
            let shared = Weak::clone(shared);
            Closure::<dyn FnMut()>::new(move || {
                if let Some(shared) = shared.upgrade() {
                    shared.post(id, PipeInput::Open);
                }
            })
        };
        let message = post(shared, id, |e: MessageEvent| {
            let data = e.data();
            if let Some(text) = data.as_string() {
                return Some(PipeInput::Text(text));
            }
            let buf = data.dyn_into::<js_sys::ArrayBuffer>().ok()?;
            Some(PipeInput::Binary(js_sys::Uint8Array::new(&buf).to_vec()))
        });
        let close = post(shared, id, |e: CloseEvent| {
            Some(PipeInput::Closed {
                code: e.code(),
                reason: e.reason(),
            })
        });
        ws.set_onopen(Some(open.as_ref().unchecked_ref()));
        ws.set_onmessage(Some(message.as_ref().unchecked_ref()));
        ws.set_onclose(Some(close.as_ref().unchecked_ref()));
        Ok(Self {
            ws,
            _open: open,
            _message: message,
            _close: close,
        })
    }
}

impl Pipe for WsPipe {
    fn send_text(&mut self, text: &str) -> Result<(), SmpError> {
        self.ws.send_with_str(text).map_err(|e| io(&e))
    }

    fn send_binary(&mut self, bytes: &[u8]) -> Result<(), SmpError> {
        self.ws.send_with_u8_array(bytes).map_err(|e| io(&e))
    }

    fn close(&mut self) {
        self.ws.set_onopen(None);
        self.ws.set_onmessage(None);
        self.ws.set_onclose(None);
        // Closing a socket that is closing already is no error worth telling.
        let _ = self.ws.close();
    }
}

impl Drop for WsPipe {
    fn drop(&mut self) {
        self.close();
    }
}
