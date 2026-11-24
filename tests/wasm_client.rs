//! The browser client under node with a fake tunnel: start, requests, events, snapshots, and a refused tunnel.
#![cfg(all(target_arch = "wasm32", feature = "wasm"))]
#![allow(clippy::unwrap_used, reason = "tests")]

use std::cell::RefCell;
use std::rc::Rc;

use serde_json::{Value, json};
use simplex_client::wasm::Client;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen_test::wasm_bindgen_test as test;

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

fn fake_tunnel(mode: &str) {
    js_sys::Function::new_no_args(FAKE)
        .call0(&JsValue::NULL)
        .unwrap();
    let t = js_sys::Reflect::get(&js_sys::global(), &"fakeTunnel".into()).unwrap();
    js_sys::Reflect::set(&t, &"mode".into(), &mode.into()).unwrap();
}

fn tunnel_state() -> Value {
    let t = js_sys::Reflect::get(&js_sys::global(), &"fakeTunnel".into()).unwrap();
    let probe = js_sys::Function::new_with_args(
        "t",
        "return { texts: t.texts, types: t.sockets.map(s => s.binaryType) }",
    );
    from_js(&probe.call1(&JsValue::NULL, &t).unwrap())
}

fn from_js(v: &JsValue) -> Value {
    serde_json::from_str(&js_sys::JSON::stringify(v).unwrap().as_string().unwrap()).unwrap()
}

fn to_js(v: &Value) -> JsValue {
    js_sys::JSON::parse(&v.to_string()).unwrap()
}

async fn sleep(ms: f64) {
    let p = js_sys::Promise::new(&mut |done, _| {
        set_timeout(&done, ms);
    });
    JsFuture::from(p).await.unwrap();
}

/// Everything a client's `name` handler hears.
struct Heard {
    list: Rc<RefCell<Vec<Value>>>,
    _f: Closure<dyn FnMut(JsValue)>,
}

impl Heard {
    fn on(client: &Client, name: &str) -> Self {
        let list = Rc::new(RefCell::new(Vec::new()));
        let l = Rc::clone(&list);
        let f =
            Closure::<dyn FnMut(JsValue)>::new(move |v: JsValue| l.borrow_mut().push(from_js(&v)));
        client.on(name, f.as_ref().unchecked_ref::<js_sys::Function>().clone());
        Self { list, _f: f }
    }

    fn all(&self) -> Vec<Value> {
        self.list.borrow().clone()
    }
}

fn start(extra: Value) -> Result<Client, JsValue> {
    let mut o = json!({ "name": "Alice", "key": "pass", "tunnel": "wss://tunnel.test/v1" });
    o.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    Client::open(&to_js(&o))
}

fn with_snapshot(bytes: &[u8], key: &str) -> Result<Client, JsValue> {
    let o = to_js(&json!({ "name": "Other", "key": key, "tunnel": "wss://tunnel.test/v1" }));
    let snap = js_sys::Uint8Array::from(bytes);
    js_sys::Reflect::set(&o, &"snapshot".into(), &snap).unwrap();
    Client::open(&o)
}

#[test]
async fn a_client_starts_offline_takes_requests_and_comes_back_from_its_snapshot() {
    fake_tunnel("silent");
    let client = start(json!({})).unwrap();
    let ready = Heard::on(&client, "ready");
    let chats = Heard::on(&client, "chats");
    let changes = Heard::on(&client, "change");
    sleep(20.0).await;
    assert_eq!(ready.all()[0]["name"], "Alice");
    assert!(!chats.all().is_empty());
    assert!(
        !changes.all().is_empty(),
        "a new store announces its first save"
    );

    client.new_group("Club").unwrap();
    sleep(20.0).await;
    let last = chats.all().pop().unwrap();
    assert_eq!(last["groups"][0]["name"], "Club", "{last}");

    let bytes = client.save().unwrap();
    client.close();
    assert!(client.save().is_err(), "a closed client saves nothing");

    let wrong = with_snapshot(&bytes, "nope").err().unwrap();
    assert_eq!(from_js(&wrong)["code"], "wrong_key");

    let again = with_snapshot(&bytes, "pass").unwrap();
    let ready = Heard::on(&again, "ready");
    let chats = Heard::on(&again, "chats");
    sleep(20.0).await;
    assert_eq!(
        ready.all()[0]["name"],
        "Alice",
        "the snapshot keeps the profile"
    );
    assert_eq!(chats.all()[0]["groups"][0]["name"], "Club");
    again.close();
}

#[test]
async fn a_refused_tunnel_reply_gives_a_clean_offline_event() {
    fake_tunnel("refuse");
    let client = start(json!({ "timeout": 300 })).unwrap();
    let offline = Heard::on(&client, "offline");
    let invite = client.create_invite();
    let answer = JsFuture::from(invite).await.err().unwrap();
    assert_eq!(from_js(&answer)["code"], "timeout");
    assert_eq!(offline.all(), [json!(true)]);

    let state = tunnel_state();
    let hello: Value = serde_json::from_str(state["texts"][0].as_str().unwrap()).unwrap();
    assert_eq!(hello["port"], 5223);
    assert!(
        hello["host"].as_str().unwrap().ends_with(".simplex.im")
            || hello["host"].as_str().unwrap().contains("simplexonflux")
    );
    assert_eq!(state["types"][0], "arraybuffer");
    client.close();
}

#[test]
async fn a_handler_may_call_the_client_and_close_rejects_waiting_calls() {
    fake_tunnel("silent");
    let client = Rc::new(start(json!({})).unwrap());
    let c = Rc::clone(&client);
    let f = Closure::<dyn FnMut(JsValue)>::new(move |_| c.set_name("Bob").unwrap());
    client.on(
        "ready",
        f.as_ref().unchecked_ref::<js_sys::Function>().clone(),
    );
    let events = Heard::on(&client, "event");
    sleep(20.0).await;
    assert_eq!(from_js(&client.profile())["name"], "Bob");
    assert!(
        events
            .all()
            .iter()
            .any(|e| e["t"] == "profile" || e["t"] == "chats")
    );

    let invite = client.create_invite();
    client.close();
    let answer = JsFuture::from(invite).await.err().unwrap();
    assert_eq!(from_js(&answer)["code"], "closed");
    let late = client
        .send(&to_js(&json!({ "friend": 1 })), "hi")
        .err()
        .unwrap();
    assert_eq!(from_js(&late)["code"], "closed");
}

#[test]
fn bad_options_are_refused_with_a_code() {
    let e = Client::open(&to_js(&json!({ "name": "A" }))).err().unwrap();
    assert_eq!(from_js(&e)["code"], "bad_option");
}
