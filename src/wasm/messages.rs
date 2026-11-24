//! Messages from JS: send, reply, edit, delete, react, read marks and history.
use wasm_bindgen::prelude::*;

use super::client::Client;
use super::convert::{chat_ref, message_ref};
use super::waiters::Want;
use crate::types::Request;

/// History pages are this long unless the caller says otherwise.
const PAGE: u32 = 50;

#[wasm_bindgen]
impl Client {
    /// Sends `text` to `{friend: id}` or `{group: id}`.
    pub fn send(&self, chat: &JsValue, text: &str) -> Result<(), JsValue> {
        self.tell(Request::Send {
            chat: chat_ref(chat)?,
            text: text.to_owned(),
            reply_to: None,
            mentions: Vec::new(),
        })
    }

    /// Answers `msg` (from a `message` event or `history`) in its conversation, quoting it.
    pub fn reply(&self, msg: &JsValue, text: &str) -> Result<(), JsValue> {
        let (chat, item) = message_ref(msg)?;
        self.tell(Request::Send {
            chat,
            text: text.to_owned(),
            reply_to: Some(item),
            mentions: Vec::new(),
        })
    }

    /// Changes the text of one of the user's own messages.
    pub fn edit(&self, msg: &JsValue, text: &str) -> Result<(), JsValue> {
        let (chat, item) = message_ref(msg)?;
        let text = text.to_owned();
        self.tell(Request::Edit { chat, item, text })
    }

    pub fn delete(&self, msg: &JsValue) -> Result<(), JsValue> {
        let (chat, item) = message_ref(msg)?;
        self.tell(Request::Delete { chat, item })
    }

    pub fn react(&self, msg: &JsValue, emoji: &str) -> Result<(), JsValue> {
        self.set_reaction(msg, emoji, true)
    }

    pub fn unreact(&self, msg: &JsValue, emoji: &str) -> Result<(), JsValue> {
        self.set_reaction(msg, emoji, false)
    }

    #[wasm_bindgen(js_name = markRead)]
    pub fn mark_read(&self, chat: &JsValue) -> Result<(), JsValue> {
        self.tell(Request::MarkRead(chat_ref(chat)?))
    }

    /// Up to `count` messages before message id `before` (or the newest), oldest first.
    pub fn history(
        &self,
        chat: &JsValue,
        before: Option<f64>,
        count: Option<u32>,
    ) -> Result<js_sys::Promise, JsValue> {
        let chat = chat_ref(chat)?;
        let before = before.map(super::convert::id).transpose()?;
        let tag = self.next_tag();
        let count = count.unwrap_or(PAGE);
        let request = Request::History {
            chat,
            before,
            count,
            tag,
        };
        Ok(self.ask(Want::History(tag), request))
    }
}

impl Client {
    fn set_reaction(&self, msg: &JsValue, emoji: &str, on: bool) -> Result<(), JsValue> {
        let (chat, item) = message_ref(msg)?;
        let emoji = emoji.to_owned();
        self.tell(Request::React {
            chat,
            item,
            emoji,
            on,
        })
    }
}
