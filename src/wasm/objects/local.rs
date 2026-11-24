//! What the user does to conversations for themselves alone, from JS: deletions here, marks, aliases, message TTLs.

use std::rc::Rc;

use wasm_bindgen::prelude::*;

use super::super::convert::{error, settled, whole};
use super::super::shared::Shared;
use super::{Friend, Group, Message};
use crate::types::Request;
use crate::types::model::ChatRef;

/// `SetMessageTtl` for `chat` (`None`: the user's own) from whole seconds or `null`.
pub(in crate::wasm) fn set_ttl(
    shared: &Rc<Shared>,
    chat: Option<ChatRef>,
    seconds: Option<f64>,
) -> js_sys::Promise {
    let ttl = match seconds.map(whole) {
        None => None,
        Some(Some(s)) => Some(s),
        Some(None) => {
            return settled(Err(error(
                "bad_argument",
                "a TTL is a whole number of seconds",
            )));
        }
    };
    shared.queue(Request::SetMessageTtl { chat, ttl })
}

macro_rules! chat_local {
    ($t:ty) => {
        #[wasm_bindgen]
        impl $t {
            /// Deletes every message here, with their files.
            #[wasm_bindgen(unchecked_return_type = "Promise<void>")]
            pub fn clear(&self) -> js_sys::Promise {
                self.shared().queue(Request::ClearChat(self.chat()))
            }

            #[wasm_bindgen(js_name = setUnreadMark, unchecked_return_type = "Promise<void>")]
            pub fn set_unread_mark(&self, on: bool) -> js_sys::Promise {
                let chat = self.chat();
                self.shared().queue(Request::SetUnreadMark { chat, on })
            }

            /// The user's own name, never sent; `""` takes it off.
            #[wasm_bindgen(js_name = setAlias, unchecked_return_type = "Promise<void>")]
            pub fn set_alias(&self, alias: &str) -> js_sys::Promise {
                let (chat, alias) = (self.chat(), alias.to_owned());
                self.shared().queue(Request::SetAlias { chat, alias })
            }

            /// Messages older than `seconds` are deleted here; `null` follows the user's own
            /// setting and `0` keeps them.
            #[wasm_bindgen(js_name = setMessageTtl, unchecked_return_type = "Promise<void>")]
            pub fn set_message_ttl(&self, seconds: Option<f64>) -> js_sys::Promise {
                set_ttl(self.shared(), Some(self.chat()), seconds)
            }
        }
    };
}

chat_local!(Friend);
chat_local!(Group);

#[wasm_bindgen]
impl Message {
    /// Deletes this message here only, with its file.
    #[wasm_bindgen(js_name = deleteForMe, unchecked_return_type = "Promise<void>")]
    pub fn delete_for_me(&self) -> js_sys::Promise {
        let (chat, item) = (self.info().chat, self.info().id);
        self.shared().queue(Request::DeleteForMe { chat, item })
    }

    /// Reads the conversation up to and including this message.
    #[wasm_bindgen(js_name = markReadUpTo, unchecked_return_type = "Promise<void>")]
    pub fn mark_read_up_to(&self) -> js_sys::Promise {
        let (chat, item) = (self.info().chat, self.info().id);
        self.shared().queue(Request::MarkReadUpTo { chat, item })
    }
}
