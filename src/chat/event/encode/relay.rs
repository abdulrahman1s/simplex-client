//! Params of channel relay and roster events (Protocol.hs `chatToAppMessage`).
use crate::chat::channel::{InlineFileInvitation, RelayCapabilities, RelayRejectionReason};
use crate::chat::ids::b64_encode;
use crate::chat::json::Params;
use serde_json::{Map, Value};

pub(super) fn acpt(relay_link: &str, relay_cap: &RelayCapabilities) -> Map<String, Value> {
    Params::new()
        .put("relayLink", &relay_link)
        .put("relayCap", relay_cap)
        .done()
}

pub(super) fn test(challenge: &[u8], signature: Option<&[u8]>) -> Map<String, Value> {
    Params::new()
        .put("challenge", &b64_encode(challenge))
        .put_opt("signature", &signature.map(b64_encode))
        .done()
}

pub(super) fn link(relay_link: &str) -> Map<String, Value> {
    Params::new().put("relayLink", &relay_link).done()
}

pub(super) fn reject(reason: &RelayRejectionReason) -> Map<String, Value> {
    Params::new().put("reason", reason).done()
}

pub(super) fn roster(version: i64, file_inv: &InlineFileInvitation) -> Map<String, Value> {
    Params::new()
        .put("version", &version)
        .put("fileInv", file_inv)
        .done()
}

pub(super) fn roster_ack(version: i64, error: Option<&String>) -> Map<String, Value> {
    Params::new()
        .put("version", &version)
        .put_opt("error", &error)
        .done()
}

pub(super) fn roster_request(version: Option<i64>) -> Map<String, Value> {
    Params::new().put_opt("version", &version).done()
}
