//! The fake tunnel, JS conversions and handler recorders the wasm client tests share.

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::{Value, json};
use simplex_client::wasm::Client;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = globalThis, js_name = setTimeout)]
    fn set_timeout(f: &js_sys::Function, ms: f64) -> JsValue;
}

/// A `WebSocket` stand-in. `silent` never opens; `refuse` opens, then answers the hello with
/// `connect_failed` and closes, as the Worker does when the relay is unreachable.
const FAKE: &str = r"
globalThis.fakeTunnel = { mode: 'silent', sockets: [], texts: [] };
globalThis.WebSocket = class {
  constructor(url) {
    this.url = url; this.binaryType = 'blob';
    const t = globalThis.fakeTunnel; t.sockets.push(this);
    if (t.mode !== 'silent') setTimeout(() => this.onopen && this.onopen({}), 0);
  }
  send(data) {
    const t = globalThis.fakeTunnel;
    if (typeof data !== 'string') return;
    t.texts.push(data);
    if (t.mode === 'refuse') setTimeout(() => {
      this.onmessage && this.onmessage({ data: JSON.stringify({ ok: false, error: 'connect_failed' }) });
      this.onclose && this.onclose({ code: 1011, reason: 'upstream' });
    }, 0);
  }
  close() { this.closed = true; }
};";

pub fn fake_tunnel(mode: &str) {
    js_sys::Function::new_no_args(FAKE)
        .call0(&JsValue::NULL)
        .unwrap();
    let t = js_sys::Reflect::get(&js_sys::global(), &"fakeTunnel".into()).unwrap();
    js_sys::Reflect::set(&t, &"mode".into(), &mode.into()).unwrap();
}

pub fn tunnel_state() -> Value {
    let t = js_sys::Reflect::get(&js_sys::global(), &"fakeTunnel".into()).unwrap();
    let probe = js_sys::Function::new_with_args(
        "t",
        "return { texts: t.texts, types: t.sockets.map(s => s.binaryType) }",
    );
    from_js(&probe.call1(&JsValue::NULL, &t).unwrap())
}

pub fn from_js(v: &JsValue) -> Value {
    let text = js_sys::JSON::stringify(v).unwrap().as_string();
    text.map_or(Value::Null, |t| serde_json::from_str(&t).unwrap())
}

/// Reads `key` of a JS object.
pub fn get(v: &JsValue, key: &str) -> JsValue {
    js_sys::Reflect::get(v, &key.into()).unwrap()
}

/// Calls `obj.method(...args)` as JS would.
pub fn call(obj: &JsValue, method: &str, args: &[JsValue]) -> JsValue {
    let f: js_sys::Function = get(obj, method).dyn_into().unwrap();
    let args: js_sys::Array = args.iter().collect();
    f.apply(obj, &args).unwrap()
}

/// Awaits a promise; its value, or what it rejected with as `Err`.
pub async fn settle(p: JsValue) -> Result<JsValue, JsValue> {
    JsFuture::from(js_sys::Promise::from(p)).await
}

pub fn to_js(v: &Value) -> JsValue {
    js_sys::JSON::parse(&v.to_string()).unwrap()
}

pub async fn sleep(ms: f64) {
    let p = js_sys::Promise::new(&mut |done, _| {
        set_timeout(&done, ms);
    });
    JsFuture::from(p).await.unwrap();
}

/// Everything a client's `name` handler hears.
pub struct Heard {
    list: Rc<RefCell<Vec<JsValue>>>,
    _f: Closure<dyn FnMut(JsValue)>,
}

impl Heard {
    pub fn on(client: &Client, name: &str) -> Self {
        let list = Rc::new(RefCell::new(Vec::new()));
        let l = Rc::clone(&list);
        let f = Closure::<dyn FnMut(JsValue)>::new(move |v: JsValue| l.borrow_mut().push(v));
        client.on(name, f.as_ref().unchecked_ref::<js_sys::Function>().clone());
        Self { list, _f: f }
    }

    /// What each call got, as JSON (`null` for nothing).
    pub fn all(&self) -> Vec<Value> {
        self.list.borrow().iter().map(from_js).collect()
    }

    /// What each call got, as the objects themselves.
    pub fn objects(&self) -> Vec<JsValue> {
        self.list.borrow().clone()
    }
}

pub fn start(extra: Value) -> Result<Client, JsValue> {
    let mut o = json!({ "name": "Alice", "key": "pass", "tunnel": "wss://tunnel.test/v1" });
    o.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    Client::open(&to_js(&o))
}

pub fn with_snapshot(bytes: &[u8], key: &str) -> Result<Client, JsValue> {
    let o = to_js(&json!({ "name": "Other", "key": key, "tunnel": "wss://tunnel.test/v1" }));
    let snap = js_sys::Uint8Array::from(bytes);
    js_sys::Reflect::set(&o, &"snapshot".into(), &snap).unwrap();
    Client::open(&o)
}
