//! What a handler or a promise gets, kept as data until JS runs, then made into objects.
use std::rc::Rc;

use serde_json::Value;
use wasm_bindgen::JsValue;

use super::convert::{num, to_js};
use super::objects::{Friend, FriendRequest, Group, GroupInvite, Member, Message};
use super::shared::Shared;
use crate::types::model::{self as wire, ChatGroup, ChatMessage, ChatRef, GroupMember};

pub(super) enum Payload {
    Nothing,
    Json(Value),
    Message(Box<ChatMessage>),
    Messages(Vec<ChatMessage>),
    Members {
        group: i64,
        list: Vec<GroupMember>,
    },
    Friend(Box<wire::Friend>),
    Group(Box<ChatGroup>),
    Invite(Box<ChatGroup>),
    Request(wire::FriendRequest),
    Member {
        group: i64,
        name: String,
    },
    /// `{ chat, member, tag, data }`, `chat` the friend or group it came in.
    Data {
        chat: ChatRef,
        member: Option<i64>,
        tag: String,
        json: Value,
    },
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
            Self::Data {
                chat,
                member,
                tag,
                json,
            } => {
                let chat: JsValue = match chat {
                    ChatRef::Direct(id) => Friend::new(s(), shared.friend(id)).into(),
                    ChatRef::Group(id) => Group::new(s(), shared.group(id)).into(),
                };
                let o = js_sys::Object::new();
                let member = member.map_or(JsValue::UNDEFINED, |m| num(m).into());
                for (k, v) in [
                    ("chat", chat),
                    ("member", member),
                    ("tag", tag.into()),
                    ("data", to_js(&json)),
                ] {
                    // Setting a key on a new plain object does not fail.
                    let _ = js_sys::Reflect::set(&o, &k.into(), &v);
                }
                o.into()
            }
        }
    }
}
