//! `message.file`: what a message carries, without fetching it.

use wasm_bindgen::prelude::*;

use super::convert::to_js;
use super::objects::Message;
use crate::types::model::MessageBody;

#[wasm_bindgen]
impl Message {
    /// The attached file as `{name, size, kind}`, or `null` for a message without one.
    #[wasm_bindgen(
        getter,
        unchecked_return_type = "{ name: string, size: number, kind: \"file\" | \"image\" | \"video\" | \"voice\" } | null"
    )]
    pub fn file(&self) -> JsValue {
        let (kind, media) = match &self.info().body {
            MessageBody::File(m) => ("file", m),
            MessageBody::Image(m) => ("image", m),
            MessageBody::Video(m) => ("video", m),
            MessageBody::Voice(m) => ("voice", m),
            MessageBody::Text(_)
            | MessageBody::Notice(_)
            | MessageBody::Gif(_)
            | MessageBody::Call(_)
            | MessageBody::Stream(_)
            | MessageBody::Deleted => return JsValue::NULL,
        };
        to_js(&serde_json::json!({ "name": media.name, "size": media.size, "kind": kind }))
    }
}
