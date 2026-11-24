//! Someone in a group, as the group's list or a message names them.
use std::rc::Rc;

use wasm_bindgen::prelude::*;

use super::super::convert::{num, plain};
use super::super::shared::Shared;
use super::{Friend, Group};
use crate::types::model::GroupMember;

#[wasm_bindgen(inspectable)]
pub struct Member {
    shared: Rc<Shared>,
    group: i64,
    name: String,
    /// From the group's member list; a message or a notice names the member only.
    info: Option<GroupMember>,
}

impl Member {
    pub(in crate::wasm) const fn named(shared: Rc<Shared>, group: i64, name: String) -> Self {
        Self {
            shared,
            group,
            name,
            info: None,
        }
    }

    pub(in crate::wasm) fn listed(shared: Rc<Shared>, group: i64, info: GroupMember) -> Self {
        let name = info.name.clone();
        Self {
            shared,
            group,
            name,
            info: Some(info),
        }
    }

    /// The client, the group, and the member's number when it came from the list.
    pub(in crate::wasm) fn parts(&self) -> (&Rc<Shared>, i64, Option<i64>) {
        (&self.shared, self.group, self.info.as_ref().map(|m| m.id))
    }
}

#[wasm_bindgen]
impl Member {
    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.name.clone()
    }

    /// The member's number in the group, when it came from `group.members()`.
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> Option<f64> {
        self.info.as_ref().map(|m| num(m.id))
    }

    #[wasm_bindgen(getter)]
    pub fn group(&self) -> Group {
        Group::new(Rc::clone(&self.shared), self.shared.group(self.group))
    }

    /// The member is also the user's friend (known from `group.members()` only).
    #[wasm_bindgen(getter)]
    pub fn friend(&self) -> Option<Friend> {
        let id = self.info.as_ref()?.friend?;
        Some(Friend::new(Rc::clone(&self.shared), self.shared.friend(id)))
    }

    /// The group's list entry as JSON; `undefined` when a message named the member.
    #[wasm_bindgen(getter)]
    pub fn raw(&self) -> JsValue {
        self.info.as_ref().map_or(JsValue::UNDEFINED, plain)
    }
}
