//! Links and forwarding from JS: `sendLink` and `forward` on a friend or group, and a message's `link` and `forwarded`.

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

    /// The preview of the link the text has: `{ uri, title, description, image }`, or `undefined`.
    #[wasm_bindgen(getter)]
    pub fn link(&self) -> JsValue {
        self.info().link.as_ref().map_or(JsValue::UNDEFINED, plain)
    }
}
