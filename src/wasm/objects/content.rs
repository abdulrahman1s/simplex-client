//! Message content from JS: `sendLink`, `sendLive`, `sendData` and `forward` on a friend or group; a message's `link`, `forwarded`, `live` and `updateLive`.

use wasm_bindgen::prelude::*;

use super::super::convert::{error, from_js, plain, settled};
use super::{Friend, Group, Message};
use crate::types::Request;
use crate::types::model::{LinkInfo, Picture};

/// `{ uri, title, description, image? }`, `image` a JPEG or PNG as bytes.
fn preview(v: &JsValue) -> Result<LinkInfo, JsValue> {
    let json = from_js(v);
    let bad = |e: &dyn std::fmt::Display| error("bad_argument", format!("not a link preview: {e}"));
    let text = |k: &str| {
        json.get(k)
            .and_then(|x| x.as_str())
            .unwrap_or_default()
            .to_owned()
    };
    if json.get("uri").and_then(|u| u.as_str()).is_none() {
        return Err(bad(&"uri is missing"));
    }
    let image = js_sys::Reflect::get(v, &"image".into())
        .ok()
        .filter(JsValue::is_object)
        .map(|i| js_sys::Uint8Array::new(&i).to_vec());
    let image = image.map(|bytes| Picture {
        mime: if bytes.starts_with(b"\x89PNG") {
            "image/png"
        } else {
            "image/jpeg"
        }
        .into(),
        bytes,
    });
    Ok(LinkInfo {
        uri: text("uri"),
        title: text("title"),
        description: text("description"),
        image,
    })
}

macro_rules! send_link {
    ($t:ty) => {
        #[wasm_bindgen]
        impl $t {
            /// Sends `message` here as the user's own, marked forwarded; its file only when it
            /// is here, else its caption.
            #[wasm_bindgen(unchecked_return_type = "Promise<void>")]
            pub fn forward(&self, message: &Message) -> js_sys::Promise {
                let (from, item) = (message.info().chat, message.info().id);
                let to = self.chat();
                self.shared().queue(Request::Forward { from, item, to })
            }

            /// Sends `data` for programs as the content type `tag`; the official apps show
            /// `fallbackText`. Nothing is kept here, and a client started with `tag` in
            /// `dataTags` hears it as `data`.
            #[wasm_bindgen(js_name = sendData, unchecked_return_type = "Promise<void>")]
            pub fn send_data(
                &self,
                tag: &str,
                #[wasm_bindgen(unchecked_param_type = "Record<string, unknown>")] data: JsValue,
                fallback_text: &str,
            ) -> js_sys::Promise {
                let serde_json::Value::Object(json) = from_js(&data) else {
                    return settled(Err(error("bad_argument", "data is an object")));
                };
                if let Some(why) = crate::chat::app_data::refusal(tag, &json) {
                    return settled(Err(error("bad_argument", why)));
                }
                self.shared().queue(Request::SendData {
                    chat: self.chat(),
                    tag: tag.to_owned(),
                    json,
                    fallback_text: fallback_text.to_owned(),
                })
            }

            /// Starts a live message here, seen as it is written; `message.updateLive` sends the rest.
            #[wasm_bindgen(js_name = sendLive, unchecked_return_type = "Promise<void>")]
            pub fn send_live(&self, text: &str) -> js_sys::Promise {
                let (chat, text) = (self.chat(), text.to_owned());
                self.shared().queue(Request::SendLive { chat, text })
            }

            /// Sends `text` with the preview of the link it has: `{ uri, title, description, image? }`.
            #[wasm_bindgen(js_name = sendLink, unchecked_return_type = "Promise<void>")]
            pub fn send_link(
                &self,
                text: &str,
                #[wasm_bindgen(unchecked_param_type = "{ uri: string, title?: string, description?: string, image?: Uint8Array }")]
                link: JsValue,
            ) -> js_sys::Promise {
                match preview(&link) {
                    Ok(preview) => self.shared().queue(Request::SendLink {
                        chat: self.chat(),
                        text: text.to_owned(),
                        preview,
                        reply_to: None,
                    }),
                    Err(e) => settled(Err(e)),
                }
            }
        }
    };
}

send_link!(Friend);
send_link!(Group);

#[wasm_bindgen]
impl Message {
    /// Forwarded from another chat.
    #[wasm_bindgen(getter)]
    pub fn forwarded(&self) -> bool {
        self.info().forwarded
    }

    /// Still being written by its author.
    #[wasm_bindgen(getter)]
    pub fn live(&self) -> bool {
        self.info().live
    }

    /// The user's live message, its text so far; `done` finishes it.
    #[wasm_bindgen(js_name = updateLive, unchecked_return_type = "Promise<void>")]
    pub fn update_live(&self, text: &str, done: bool) -> js_sys::Promise {
        let (chat, item) = (self.info().chat, self.info().id);
        let text = text.to_owned();
        self.shared().queue(Request::UpdateLive {
            chat,
            item,
            text,
            done,
        })
    }

    /// The preview of the link the text has: `{ uri, title, description, image }`, or `undefined`.
    #[wasm_bindgen(getter)]
    pub fn link(&self) -> JsValue {
        self.info().link.as_ref().map_or(JsValue::UNDEFINED, plain)
    }
}
