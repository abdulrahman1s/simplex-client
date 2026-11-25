//! `sendFile`'s arguments: the kind of file, and `{ caption, duration, preview }`.

use wasm_bindgen::prelude::*;

use super::convert::{error, from_js};
use crate::types::FileKind;

pub(super) fn kind(kind: Option<String>) -> Result<FileKind, JsValue> {
    let Some(kind) = kind else {
        return Ok(FileKind::File);
    };
    serde_json::from_value(serde_json::Value::String(kind)).map_err(|_| {
        error(
            "bad_argument",
            "kind is \"image\", \"gif\", \"video\", \"voice\" or \"file\"",
        )
    })
}

/// `sendFile`'s last argument: `{ caption, duration, preview }`, each optional; `preview` is a
/// small JPEG or PNG for a picture or video.
#[derive(Default, serde::Deserialize)]
pub(super) struct FileOptions {
    #[serde(default)]
    pub(super) caption: String,
    #[serde(default)]
    pub(super) duration: Option<u32>,
    #[serde(default)]
    pub(super) preview: Option<Vec<u8>>,
}

pub(super) fn options(v: &JsValue) -> Result<FileOptions, JsValue> {
    if v.is_undefined() || v.is_null() {
        return Ok(FileOptions::default());
    }
    let mut json = from_js(v);
    // A Uint8Array reads as an object of numbered keys; take its bytes as a list.
    if let Some(p) = js_sys::Reflect::get(v, &"preview".into())
        .ok()
        .filter(|p| p.is_object())
    {
        json["preview"] = serde_json::to_value(js_sys::Uint8Array::new(&p).to_vec())
            .unwrap_or(serde_json::Value::Null);
    }
    serde_json::from_value(json)
        .map_err(|e| error("bad_argument", format!("not file options: {e}")))
}
