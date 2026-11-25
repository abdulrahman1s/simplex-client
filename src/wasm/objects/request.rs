//! Someone asking to connect through the user's address: accept or reject.
use std::rc::Rc;

use wasm_bindgen::prelude::*;

use super::super::convert::num;
use super::super::shared::Shared;
use crate::types::Request;
use crate::types::model as wire;

#[wasm_bindgen(inspectable)]
pub struct FriendRequest {
    shared: Rc<Shared>,
    info: wire::FriendRequest,
}

impl FriendRequest {
    pub(in crate::wasm) const fn new(shared: Rc<Shared>, info: wire::FriendRequest) -> Self {
        Self { shared, info }
    }
}

#[wasm_bindgen]
impl FriendRequest {
    #[wasm_bindgen(getter)]
    pub fn id(&self) -> f64 {
        num(self.info.id)
    }

    /// The name they gave.
    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.info.name.clone()
    }

    /// They become a friend; `friendAdd` follows once connected.
    #[wasm_bindgen(unchecked_return_type = "Promise<void>")]
    pub fn accept(&self) -> js_sys::Promise {
        self.shared.queue(Request::Accept(self.info.id))
    }

    /// Accepts by a random name only: they never see the user's profile.
    #[wasm_bindgen(js_name = acceptIncognito, unchecked_return_type = "Promise<void>")]
    pub fn accept_incognito(&self) -> js_sys::Promise {
        self.shared.queue(Request::AcceptIncognito(self.info.id))
    }

    #[wasm_bindgen(unchecked_return_type = "Promise<void>")]
    pub fn reject(&self) -> js_sys::Promise {
        self.shared.queue(Request::Reject(self.info.id))
    }
}
