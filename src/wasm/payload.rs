//! What a handler or a promise gets, kept as data until JS runs, then made into objects.
use std::rc::Rc;

use serde_json::Value;
use wasm_bindgen::JsValue;

use super::convert::to_js;
use super::objects::{Friend, FriendRequest, Group, GroupInvite, Member, Message};
use super::shared::Shared;
use crate::types::model::{self as wire, ChatGroup, ChatMessage, GroupMember};

pub(super) enum Payload {
    Nothing,
    Json(Value),
    Message(Box<ChatMessage>),
    Messages(Vec<ChatMessage>),
    Members { group: i64, list: Vec<GroupMember> },
    Friend(Box<wire::Friend>),
    Group(Box<ChatGroup>),
    Invite(Box<ChatGroup>),
    Request(wire::FriendRequest),
    Member { group: i64, name: String },
}

impl Payload {
    pub(super) fn into_js(self, shared: &Rc<Shared>) -> JsValue {
        let s = || Rc::clone(shared);
        match self {
            Self::Nothing => JsValue::UNDEFINED,
            Self::Json(v) => to_js(&v),
            Self::Message(m) => Message::new(s(), *m).into(),
            Self::Messages(list) => {
                let list = list
                    .into_iter()
                    .map(|m| JsValue::from(Message::new(s(), m)));
                list.collect::<js_sys::Array>().into()
            }
            Self::Members { group, list } => {
                let list = list.into_iter().map(|m| Member::listed(s(), group, m));
                list.map(JsValue::from).collect::<js_sys::Array>().into()
            }
            Self::Friend(f) => Friend::new(s(), *f).into(),
            Self::Group(g) => Group::new(s(), *g).into(),
            Self::Invite(g) => GroupInvite::new(s(), *g).into(),
            Self::Request(r) => FriendRequest::new(s(), r).into(),
            Self::Member { group, name } => Member::named(s(), group, name).into(),
        }
    }
}
