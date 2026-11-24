//! `Client.start`'s options object: who, which key, which tunnel, and what to start from.
use wasm_bindgen::{JsCast, JsValue};

use super::convert::error;

/// How long a call that waits for an answer waits, as the native client does.
const TIMEOUT_MS: i64 = 60_000;

pub(super) struct Options {
    pub(super) name: String,
    pub(super) key: String,
    /// `wss://<worker>/v1`.
    pub(super) tunnel: String,
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
            tunnel: text(o, "tunnel")?.ok_or_else(|| need("tunnel"))?,
            token: text(o, "tunnelToken")?,
            relays,
            snapshot,
            #[allow(clippy::cast_possible_truncation, reason = "milliseconds")]
            timeout_ms: timeout_ms.map_or(TIMEOUT_MS, |t| t as i64),
            log: get(o, "log").and_then(|v| v.as_bool()).unwrap_or(false),
        })
    }
}
