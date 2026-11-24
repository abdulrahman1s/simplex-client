//! A group from JS: write to it, invite friends, list members, leave.
use std::rc::Rc;

use wasm_bindgen::prelude::*;

use super::super::convert::{num, plain};
use super::super::shared::Shared;
use super::super::waiters::Want;
use super::Friend;
use super::page::{HistoryOptions, history};
use crate::types::Request;
use crate::types::model::{ChatGroup, ChatRef};

#[wasm_bindgen(inspectable)]
pub struct Group {
    shared: Rc<Shared>,
    info: ChatGroup,
}

impl Group {
    pub(in crate::wasm) const fn new(shared: Rc<Shared>, info: ChatGroup) -> Self {
        Self { shared, info }
    }

    pub(in crate::wasm) const fn shared(&self) -> &Rc<Shared> {
        &self.shared
    }

    pub(in crate::wasm) const fn chat(&self) -> ChatRef {
        self.info.chat()
    }
}

#[wasm_bindgen]
impl Group {
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> f64 {
        num(self.info.id)
    }

    /// Empty when the chat list did not have this group yet.
    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.info.name.clone()
    }

    /// `invited`, `joining`, `member` or `left`.
    #[wasm_bindgen(
        getter,
        unchecked_return_type = "\"invited\" | \"joining\" | \"member\" | \"left\""
    )]
    pub fn membership(&self) -> JsValue {
        plain(&self.info.membership)
    }

    /// The user owns the group: they may invite.
    #[wasm_bindgen(getter)]
    pub fn owner(&self) -> bool {
        self.info.owner
    }

    /// Everything the chat list says about this group, as JSON.
    #[wasm_bindgen(getter)]
    pub fn raw(&self) -> JsValue {
        plain(&self.info)
    }

    /// Resolves once the message is queued; `message` reports it.
    #[wasm_bindgen(unchecked_return_type = "Promise<void>")]
    pub fn send(&self, text: &str) -> js_sys::Promise {
        self.shared.queue(Request::Send {
            chat: self.chat(),
            text: text.to_owned(),
            reply_to: None,
            mentions: Vec::new(),
        })
    }

    /// Invites a friend; the user must own the group.
    #[wasm_bindgen(js_name = addMember, unchecked_return_type = "Promise<void>")]
    pub fn add_member(&self, friend: &Friend) -> js_sys::Promise {
        let (group, friend) = (self.info.id, friend.key());
        self.shared.queue(Request::AddMember { group, friend })
    }

    #[wasm_bindgen(unchecked_return_type = "Promise<void>")]
    pub fn leave(&self) -> js_sys::Promise {
        self.shared.queue(Request::LeaveGroup(self.info.id))
    }

    /// The user first, then everyone still in the group.
    #[wasm_bindgen(unchecked_return_type = "Promise<Member[]>")]
    pub fn members(&self) -> js_sys::Promise {
        let id = self.info.id;
        self.shared.ask(Want::Members(id), Request::Members(id))
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
