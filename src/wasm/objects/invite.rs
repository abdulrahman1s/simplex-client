//! An invitation into a group: join it or turn it down.
use std::rc::Rc;

use wasm_bindgen::prelude::*;

use super::super::shared::Shared;
use super::Group;
use crate::types::Request;
use crate::types::model::ChatGroup;

#[wasm_bindgen(inspectable)]
pub struct GroupInvite {
    shared: Rc<Shared>,
    info: ChatGroup,
}

impl GroupInvite {
    pub(in crate::wasm) const fn new(shared: Rc<Shared>, info: ChatGroup) -> Self {
        Self { shared, info }
    }
}

#[wasm_bindgen]
impl GroupInvite {
    /// The group, as the invitation describes it.
    #[wasm_bindgen(getter)]
    pub fn group(&self) -> Group {
        Group::new(Rc::clone(&self.shared), self.info.clone())
    }

    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.info.name.clone()
    }

    /// Joins; `groupAdd` follows once connected.
    #[wasm_bindgen(unchecked_return_type = "Promise<void>")]
    pub fn accept(&self) -> js_sys::Promise {
        self.shared.queue(Request::JoinGroup(self.info.id))
    }

    /// Declining is leaving before joining: the group stays listed as left.
    #[wasm_bindgen(unchecked_return_type = "Promise<void>")]
    pub fn reject(&self) -> js_sys::Promise {
        self.shared.queue(Request::LeaveGroup(self.info.id))
    }
}
