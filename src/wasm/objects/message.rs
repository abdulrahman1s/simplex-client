//! A message from JS: answer it, react to it, change or delete it.
use std::rc::Rc;

use wasm_bindgen::prelude::*;

use super::super::convert::{num, plain};
use super::super::shared::Shared;
use super::{Friend, Group, Member};
use crate::types::Request;
use crate::types::model::{ChatMessage, ChatRef, MessageAuthor};

#[wasm_bindgen(inspectable)]
pub struct Message {
    shared: Rc<Shared>,
    info: ChatMessage,
}

impl Message {
    pub(in crate::wasm) const fn new(shared: Rc<Shared>, info: ChatMessage) -> Self {
        Self { shared, info }
    }

    fn friend(&self, id: i64) -> JsValue {
        Friend::new(Rc::clone(&self.shared), self.shared.friend(id)).into()
    }

    fn react_with(&self, emoji: &str, on: bool) -> js_sys::Promise {
        let (chat, item, emoji) = (self.info.chat, self.info.id, emoji.to_owned());
        self.shared.queue(Request::React {
            chat,
            item,
            emoji,
            on,
        })
    }
}

#[wasm_bindgen]
impl Message {
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> f64 {
        num(self.info.id)
    }

    /// What the sender wrote: the text, or a file's caption; empty for notices and calls.
    #[wasm_bindgen(getter)]
    pub fn text(&self) -> String {
        self.info.text().to_owned()
    }

    /// Sent by the user.
    #[wasm_bindgen(getter)]
    pub fn mine(&self) -> bool {
        self.info.mine()
    }

    /// Milliseconds since the epoch.
    #[wasm_bindgen(getter)]
    pub fn at(&self) -> f64 {
        num(self.info.at)
    }

    /// The quoted message's id, when this one is a reply.
    #[wasm_bindgen(getter, js_name = replyTo)]
    pub fn reply_to(&self) -> Option<f64> {
        self.info.reply.as_ref()?.id.map(num)
    }

    /// Where it is: a `Friend` or a `Group`.
    #[wasm_bindgen(getter, unchecked_return_type = "Friend | Group")]
    pub fn chat(&self) -> JsValue {
        match self.info.chat {
            ChatRef::Direct(id) => self.friend(id),
            ChatRef::Group(id) => Group::new(Rc::clone(&self.shared), self.shared.group(id)).into(),
        }
    }

    /// Who wrote it: a `Friend` or a `Member`; `null` for the user's own.
    #[wasm_bindgen(getter, unchecked_return_type = "Friend | Member | null")]
    pub fn author(&self) -> JsValue {
        let member = |name: &str, g| Member::named(Rc::clone(&self.shared), g, name.to_owned());
        match (&self.info.author, self.info.chat) {
            (MessageAuthor::Me, _) => JsValue::NULL,
            (MessageAuthor::Friend | MessageAuthor::Member(_), ChatRef::Direct(id)) => {
                self.friend(id)
            }
            (MessageAuthor::Friend, ChatRef::Group(g)) => member("", g).into(),
            (MessageAuthor::Member(name), ChatRef::Group(g)) => member(name, g).into(),
        }
    }

    /// The whole message as JSON: body, delivery, reactions, quote, mentions.
    #[wasm_bindgen(getter)]
    pub fn raw(&self) -> JsValue {
        plain(&self.info)
    }

    /// Answers in the same conversation, quoting this message.
    #[wasm_bindgen(unchecked_return_type = "Promise<void>")]
    pub fn reply(&self, text: &str) -> js_sys::Promise {
        self.shared.queue(Request::Send {
            chat: self.info.chat,
            text: text.to_owned(),
            reply_to: Some(self.info.id),
            mentions: Vec::new(),
        })
    }

    #[wasm_bindgen(unchecked_return_type = "Promise<void>")]
    pub fn react(&self, emoji: &str) -> js_sys::Promise {
        self.react_with(emoji, true)
    }

    #[wasm_bindgen(unchecked_return_type = "Promise<void>")]
    pub fn unreact(&self, emoji: &str) -> js_sys::Promise {
        self.react_with(emoji, false)
    }

    /// Changes the text; only the user's own messages.
    #[wasm_bindgen(unchecked_return_type = "Promise<void>")]
    pub fn edit(&self, text: &str) -> js_sys::Promise {
        let (chat, item, text) = (self.info.chat, self.info.id, text.to_owned());
        self.shared.queue(Request::Edit { chat, item, text })
    }

    #[wasm_bindgen(unchecked_return_type = "Promise<void>")]
    pub fn delete(&self) -> js_sys::Promise {
        let (chat, item) = (self.info.chat, self.info.id);
        self.shared.queue(Request::Delete { chat, item })
    }
}
