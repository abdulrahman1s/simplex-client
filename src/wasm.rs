//! JavaScript bindings for the pure protocol core: links, chat messages, keys and randomness.
use std::fmt::Display;

use serde_json::{Value, json};
use wasm_bindgen::prelude::*;

use crate::agent::links::{ConnShortLink, ConnectionLink, ConnectionRequestUri, ContactConnType};
use crate::chat::{ChatMessage, encode_chat_message, parse_chat_messages};
use crate::crypto::{Alg, KeyPair, PrivateKey, PublicKey, Signature, random_vec};

fn js_err(e: impl Display) -> JsError {
    JsError::new(&e.to_string())
}

fn to_js(value: &Value) -> Result<JsValue, JsError> {
    js_sys::JSON::parse(&value.to_string()).map_err(|_| JsError::new("not JSON"))
}

fn alg(name: &str) -> Result<Alg, JsError> {
    match name.to_ascii_lowercase().as_str() {
        "ed25519" => Ok(Alg::Ed25519),
        "ed448" => Ok(Alg::Ed448),
        "x25519" => Ok(Alg::X25519),
        "x448" => Ok(Alg::X448),
        _ => Err(JsError::new(
            "unknown algorithm: use ed25519, ed448, x25519 or x448",
        )),
    }
}

fn contact_kind(t: ContactConnType) -> &'static str {
    match t {
        ContactConnType::Contact => "contact",
        ContactConnType::Channel => "channel",
        ContactConnType::Group => "group",
        ContactConnType::Relay => "relay",
    }
}

/// A full contact link is a group's when its client data names a `groupLinkId`.
fn full_kind(link: &ConnectionRequestUri) -> &'static str {
    match link {
        ConnectionRequestUri::Invitation { .. } => "invitation",
        ConnectionRequestUri::Contact { data, .. } => {
            let group = data
                .client_data
                .as_deref()
                .and_then(|d| serde_json::from_str::<Value>(d).ok())
                .is_some_and(|v| v.get("groupLinkId").is_some());
            if group { "group" } else { "contact" }
        }
    }
}

/// What a pasted SimpleX link is: `{ kind, short, servers, link }`. `kind` is `invitation`,
/// `contact`, `group`, `channel` or `relay`; `servers` are SMP addresses (a short link of a
/// preset server names only its host); `link` is the link written back in its canonical form.
#[wasm_bindgen(js_name = parseLink)]
pub fn parse_link(link: &str) -> Result<JsValue, JsError> {
    let parsed = ConnectionLink::parse(link.trim()).map_err(js_err)?;
    let (kind, short, servers) = match &parsed {
        ConnectionLink::Full(full) => {
            let servers = full.data().smp_queues.iter();
            let servers = servers.map(|q| q.address.server.to_string()).collect();
            (full_kind(full), false, servers)
        }
        ConnectionLink::Short(s) => {
            let kind = match s {
                ConnShortLink::Invitation { .. } => "invitation",
                ConnShortLink::Contact { contact_type, .. } => contact_kind(*contact_type),
            };
            (kind, true, vec![s.server().to_string()])
        }
    };
    to_js(&json!({ "kind": kind, "short": short, "servers": servers, "link": parsed.to_uri() }))
}

/// The body to send for a chat message given as JSON (`{ v?, msgId?, event, params }`);
/// an `X` container when it is over the size limit, which is lower with post-quantum keys.
#[wasm_bindgen(js_name = encodeChatMessage)]
pub fn encode_message(json: &str, pq: bool) -> Result<Vec<u8>, JsError> {
    let msg = ChatMessage::from_json(json.as_bytes()).map_err(js_err)?;
    encode_chat_message(&msg, pq).map_err(js_err)
}

/// Every chat message in a received body (JSON, a batch or an `X` container), as objects.
#[wasm_bindgen(js_name = parseChatMessages)]
pub fn parse_messages(body: &[u8]) -> Result<JsValue, JsError> {
    let msgs = parse_chat_messages(body).map_err(js_err)?;
    to_js(&Value::Array(
        msgs.iter().map(ChatMessage::to_value).collect(),
    ))
}

/// A key pair as DER: X.509 SubjectPublicKeyInfo and PKCS#8.
#[wasm_bindgen(getter_with_clone)]
pub struct Keys {
    #[wasm_bindgen(js_name = publicKey)]
    pub public_key: Vec<u8>,
    #[wasm_bindgen(js_name = privateKey)]
    pub private_key: Vec<u8>,
}

/// A new key pair; `alg` is `ed25519`, `ed448`, `x25519` or `x448`.
#[wasm_bindgen(js_name = generateKeyPair)]
pub fn generate_key_pair(alg_name: &str) -> Result<Keys, JsError> {
    let pair = KeyPair::generate(alg(alg_name)?).map_err(js_err)?;
    Ok(Keys {
        public_key: pair.public.to_der(),
        private_key: pair.private.to_der().to_vec(),
    })
}

/// Signs with an Ed25519 or Ed448 private key in PKCS#8 DER.
#[wasm_bindgen]
pub fn sign(private_key: &[u8], message: &[u8]) -> Result<Vec<u8>, JsError> {
    let key = PrivateKey::from_der(private_key).map_err(js_err)?;
    Ok(key.sign(message).map_err(js_err)?.as_bytes().to_vec())
}

/// Checks a signature against a public key in X.509 DER; false for any malformed input.
#[wasm_bindgen]
#[must_use]
pub fn verify(public_key: &[u8], signature: &[u8], message: &[u8]) -> bool {
    PublicKey::from_der(public_key).is_ok_and(|key| {
        Signature::from_raw(key.alg(), signature).is_ok_and(|s| key.verify(&s, message))
    })
}

/// `n` bytes from the platform's secure random source (`crypto.getRandomValues` in JS).
#[wasm_bindgen(js_name = randomBytes)]
pub fn random_bytes(n: usize) -> Result<Vec<u8>, JsError> {
    random_vec(n).map_err(js_err)
}
