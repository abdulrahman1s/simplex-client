//! `Client.start`'s options object: who, which key, which tunnel, and what to start from.
use wasm_bindgen::{JsCast, JsValue};

use super::convert::error;

/// How long a call that waits for an answer waits, as the native client does.
const TIMEOUT_MS: i64 = 60_000;

/// How relays are reached.
pub(super) enum Transport {
    /// The WebSocket byte tunnel at this `wss://…/v1` URL.
    Tunnel(String),
    /// Straight TCP from Node.js or Bun.
    Tcp,
}

/// Node.js, or Bun, which reports a Node.js version too.
fn in_node() -> bool {
    let process = js_sys::Reflect::get(&js_sys::global(), &"process".into()).ok();
    let versions = process.and_then(|p| js_sys::Reflect::get(&p, &"versions".into()).ok());
    let node = versions.and_then(|v| js_sys::Reflect::get(&v, &"node".into()).ok());
    node.is_some_and(|n| n.is_string())
}

/// `transport: "tcp"`, or a `tunnel` URL; on Node.js or Bun with neither, TCP.
fn transport(o: &JsValue) -> Result<Transport, JsValue> {
    let tunnel = text(o, "tunnel")?;
    match (text(o, "transport")?.as_deref(), tunnel) {
        (Some("tcp"), _) => Ok(Transport::Tcp),
        (Some("tunnel") | None, Some(url)) => Ok(Transport::Tunnel(url)),
        (Some("tunnel"), None) => Err(error(
            "bad_option",
            "the tunnel transport needs a tunnel URL",
        )),
        (None, None) if in_node() => Ok(Transport::Tcp),
        (None, None) => Err(error(
            "bad_option",
            "a browser reaches relays only through a tunnel: pass tunnel: \"wss://…/v1\"",
        )),
        (Some(other), _) => Err(error(
            "bad_option",
            format!("transport is \"tcp\" or \"tunnel\", not {other:?}"),
        )),
    }
}

pub(super) struct Options {
    pub(super) name: String,
    pub(super) key: String,
    pub(super) transport: Transport,
    pub(super) token: Option<String>,
    /// SMP relay addresses; empty means the presets.
    pub(super) relays: Vec<String>,
    /// Bytes from `client.save()`; none starts an empty store.
    pub(super) snapshot: Option<Vec<u8>>,
    pub(super) timeout_ms: i64,
    /// Write the engine's log to the console.
    pub(super) log: bool,
}

fn get(o: &JsValue, key: &str) -> Option<JsValue> {
    let v = js_sys::Reflect::get(o, &key.into()).ok()?;
    (!v.is_undefined() && !v.is_null()).then_some(v)
}

fn text(o: &JsValue, key: &str) -> Result<Option<String>, JsValue> {
    get(o, key)
        .map(|v| {
            v.as_string()
                .ok_or_else(|| error("bad_option", format!("{key} is not a string")))
        })
        .transpose()
}

impl Options {
    pub(super) fn parse(o: &JsValue) -> Result<Self, JsValue> {
        let need = |k: &str| error("bad_option", format!("{k} is required"));
        let relays = match get(o, "relays") {
            Some(list) => js_sys::Array::from(&list)
                .iter()
                .map(|r| {
                    r.as_string()
                        .ok_or_else(|| error("bad_option", "relays are strings"))
                })
                .collect::<Result<_, _>>()?,
            None => Vec::new(),
        };
        let snapshot = match get(o, "snapshot") {
            Some(v) => Some(
                v.dyn_into::<js_sys::Uint8Array>()
                    .map_err(|_| error("bad_option", "snapshot is a Uint8Array"))?
                    .to_vec(),
            ),
            None => None,
        };
        let timeout_ms = get(o, "timeout").and_then(|v| v.as_f64());
        Ok(Self {
            name: text(o, "name")?.unwrap_or_else(|| "SimpleX user".into()),
            key: text(o, "key")?.ok_or_else(|| need("key"))?,
            transport: transport(o)?,
            token: text(o, "tunnelToken")?,
            relays,
            snapshot,
            #[allow(clippy::cast_possible_truncation, reason = "milliseconds")]
            timeout_ms: timeout_ms.map_or(TIMEOUT_MS, |t| t as i64),
            log: get(o, "log").and_then(|v| v.as_bool()).unwrap_or(false),
        })
    }
}
