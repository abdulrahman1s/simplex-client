//! A friend from JS: write to them, read your history, compare codes, remove them.
use std::rc::Rc;

use wasm_bindgen::prelude::*;

use super::super::convert::{num, plain};
use super::super::shared::Shared;
use super::super::waiters::Want;
use super::page::{HistoryOptions, history};
use crate::types::Request;
use crate::types::model::{self as wire, ChatRef};

#[wasm_bindgen(inspectable)]
pub struct Friend {
    shared: Rc<Shared>,
    info: wire::Friend,
}

impl Friend {
    pub(in crate::wasm) const fn new(shared: Rc<Shared>, info: wire::Friend) -> Self {
        Self { shared, info }
    }

    pub(in crate::wasm) const fn key(&self) -> i64 {
        self.info.id
    }

    pub(in crate::wasm) const fn shared(&self) -> &Rc<Shared> {
        &self.shared
    }

    pub(in crate::wasm) const fn chat(&self) -> ChatRef {
        self.info.chat()
    }
}

#[wasm_bindgen]
impl Friend {
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> f64 {
        num(self.info.id)
    }

    /// Empty when the chat list did not have this friend yet.
    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.info.name.clone()
    }

    /// Connected: messages can be sent.
    #[wasm_bindgen(getter)]
    pub fn ready(&self) -> bool {
        self.info.ready
    }

    #[wasm_bindgen(getter)]
    pub fn verified(&self) -> bool {
        self.info.verified
    }

    /// Everything the chat list says about this friend, as JSON.
    #[wasm_bindgen(getter)]
    pub fn raw(&self) -> JsValue {
        plain(&self.info)
    }

    /// Resolves once the message is queued; `message` reports it, `messageUpdate` its delivery.
    #[wasm_bindgen(unchecked_return_type = "Promise<void>")]
    pub fn send(&self, text: &str) -> js_sys::Promise {
        self.shared.queue(Request::Send {
            chat: self.chat(),
            text: text.to_owned(),
            reply_to: None,
            mentions: Vec::new(),
        })
    }

    /// The connection is deleted on both sides; the conversation goes with it.
    #[wasm_bindgen(unchecked_return_type = "Promise<void>")]
    pub fn remove(&self) -> js_sys::Promise {
        self.shared.queue(Request::RemoveFriend(self.info.id))
    }

    /// The security code to compare with the friend's, out of band.
    #[wasm_bindgen(js_name = showCode, unchecked_return_type = "Promise<string>")]
    pub fn show_code(&self) -> js_sys::Promise {
        let id = self.info.id;
        self.shared.ask(Want::Code(id), Request::ShowCode(id))
    }

    /// Messages, oldest first: the newest `limit` (50), or those before message `before`.
    #[wasm_bindgen(unchecked_return_type = "Promise<Message[]>")]
    pub fn history(&self, options: Option<HistoryOptions>) -> js_sys::Promise {
        history(&self.shared, self.chat(), options)
    }

    #[wasm_bindgen(js_name = markRead, unchecked_return_type = "Promise<void>")]
    pub fn mark_read(&self) -> js_sys::Promise {
        self.shared.queue(Request::MarkRead(self.chat()))
    }
}
