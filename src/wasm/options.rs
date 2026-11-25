//! `Client.start`'s options object: who, which key, which tunnel, and what to start from.
use std::collections::BTreeSet;

use wasm_bindgen::{JsCast, JsValue};
use zeroize::Zeroizing;

use super::convert::error;
use crate::limits::is_valid_name;
use crate::types::{ErrorCode, ProxyFallback, ProxyMode};

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
    /// Wiped when the options are dropped, once the store is open.
    pub(super) key: Zeroizing<String>,
    pub(super) transport: Transport,
    pub(super) token: Option<String>,
    /// SMP relay addresses; empty means the presets.
    pub(super) relays: Vec<String>,
    /// Bytes from `client.save()`; none starts an empty store.
    pub(super) snapshot: Option<Vec<u8>>,
    /// Bytes from each `client.saveChanges()` after it, oldest first.
    pub(super) changes: Vec<Vec<u8>>,
    pub(super) timeout_ms: i64,
    /// Write the engine's log to the console.
    pub(super) log: bool,
    pub(super) proxy_mode: ProxyMode,
    pub(super) proxy_fallback: ProxyFallback,
    /// Content types heard as `data` instead of `message`.
    pub(super) data_tags: BTreeSet<String>,
    /// Hear what changed in the chat list (`friendUpdate`, `groupUpdate`) instead of whole lists.
    pub(super) roster_deltas: bool,
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

/// A boolean under `key`; false when left out.
fn flag(o: &JsValue, key: &str) -> Result<bool, JsValue> {
    get(o, key).map_or(Ok(false), |v| {
        v.as_bool()
            .ok_or_else(|| error("bad_option", format!("{key} is true or false")))
    })
}

/// An array of strings under `key`, or nothing.
fn strings(o: &JsValue, key: &str) -> Result<Vec<String>, JsValue> {
    let Some(list) = get(o, key) else {
        return Ok(Vec::new());
    };
    js_sys::Array::from(&list)
        .iter()
        .map(|r| {
            r.as_string()
                .ok_or_else(|| error("bad_option", format!("{key} are strings")))
        })
        .collect()
}

/// `changes`: an array of `Uint8Array`s, or nothing.
fn changes(o: &JsValue) -> Result<Vec<Vec<u8>>, JsValue> {
    let Some(list) = get(o, "changes") else {
        return Ok(Vec::new());
    };
    let bad = || error("bad_option", "changes is an array of Uint8Arrays");
    if !js_sys::Array::is_array(&list) {
        return Err(bad());
    }
    js_sys::Array::from(&list)
        .iter()
        .map(|c| {
            c.dyn_into::<js_sys::Uint8Array>()
                .map(|a| a.to_vec())
                .map_err(|_| bad())
        })
        .collect()
}

impl Options {
    pub(super) fn parse(o: &JsValue) -> Result<Self, JsValue> {
        let need = |k: &str| error("bad_option", format!("{k} is required"));
        let relays = strings(o, "relays")?;
        let snapshot = match get(o, "snapshot") {
            Some(v) => Some(
                v.dyn_into::<js_sys::Uint8Array>()
                    .map_err(|_| error("bad_option", "snapshot is a Uint8Array"))?
                    .to_vec(),
            ),
            None => None,
        };
        let changes = changes(o)?;
        if snapshot.is_none() && !changes.is_empty() {
            return Err(error("bad_option", "changes need the snapshot they follow"));
        }
        let timeout_ms = get(o, "timeout").and_then(|v| v.as_f64());
        let proxy_mode = match text(o, "privateRouting")? {
            Some(m) => m
                .parse()
                .map_err(|_| error("bad_option", "privateRouting is always, unknown or never"))?,
            None => ProxyMode::default(),
        };
        let proxy_fallback = match get(o, "allowDirect").and_then(|v| v.as_bool()) {
            Some(true) => ProxyFallback::Allow,
            Some(false) | None => ProxyFallback::AllowProtected,
        };
        let name = text(o, "name")?.unwrap_or_else(|| "SimpleX user".into());
        // Only a new store's user takes the name (Commands.hs `CreateActiveUser`: `checkValidName`).
        if snapshot.is_none() && !is_valid_name(name.trim()) {
            let code = ErrorCode::InvalidName.name();
            return Err(error(code, format!("not a valid name: {name:?}")));
        }
        Ok(Self {
            name,
            key: Zeroizing::new(
                text(o, "key")?
                    .filter(|k| !k.is_empty())
                    .ok_or_else(|| need("key"))?,
            ),
            transport: transport(o)?,
            token: text(o, "tunnelToken")?,
            relays,
            snapshot,
            changes,
            #[allow(clippy::cast_possible_truncation, reason = "milliseconds")]
            timeout_ms: timeout_ms.map_or(TIMEOUT_MS, |t| t as i64),
            log: get(o, "log").and_then(|v| v.as_bool()).unwrap_or(false),
            proxy_mode,
            proxy_fallback,
            data_tags: strings(o, "dataTags")?.into_iter().collect(),
            roster_deltas: flag(o, "rosterDeltas")?,
        })
    }
}
