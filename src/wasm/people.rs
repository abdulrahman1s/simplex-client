//! Friends, groups and the user from JS: links, requests, codes, groups and the name.
use wasm_bindgen::prelude::*;

use super::client::Client;
use super::convert::{error, id};
use super::waiters::Want;
use crate::types::Request;

#[wasm_bindgen]
impl Client {
    /// A one-time invitation link for one person.
    #[wasm_bindgen(js_name = createInvite)]
    pub fn create_invite(&self) -> js_sys::Promise {
        self.ask(Want::Invite, Request::CreateInvite)
    }

    /// The user's long-lived address, made on first use.
    #[wasm_bindgen(js_name = createAddress)]
    pub fn create_address(&self) -> js_sys::Promise {
        self.ask(Want::Address, Request::CreateAddress)
    }

    #[wasm_bindgen(js_name = deleteAddress)]
    pub fn delete_address(&self) -> Result<(), JsValue> {
        self.tell(Request::DeleteAddress)
    }

    /// Connects through an invitation, an address or a group link.
    pub fn connect(&self, link: &str) -> Result<(), JsValue> {
        self.tell(Request::Connect(link.trim().to_owned()))
    }

    /// Accepts a request from a `request` event (`req.id`).
    pub fn accept(&self, request: f64) -> Result<(), JsValue> {
        self.tell(Request::Accept(id(request)?))
    }

    pub fn reject(&self, request: f64) -> Result<(), JsValue> {
        self.tell(Request::Reject(id(request)?))
    }

    #[wasm_bindgen(js_name = removeFriend)]
    pub fn remove_friend(&self, friend: f64) -> Result<(), JsValue> {
        self.tell(Request::RemoveFriend(id(friend)?))
    }

    /// The security code to compare with the friend's, out of band.
    #[wasm_bindgen(js_name = showCode)]
    pub fn show_code(&self, friend: f64) -> Result<js_sys::Promise, JsValue> {
        let friend = id(friend)?;
        Ok(self.ask(Want::Code(friend), Request::ShowCode(friend)))
    }

    #[wasm_bindgen(js_name = setName)]
    pub fn set_name(&self, name: &str) -> Result<(), JsValue> {
        let profile = {
            let mut core = self.shared.core.borrow_mut();
            let sugar = core.as_mut().map(|c| &mut c.sugar);
            let p = sugar.and_then(|s| s.profile.as_mut());
            let p = p.ok_or_else(|| error("not_ready", "the profile is not known yet"))?;
            name.clone_into(&mut p.name);
            p.clone()
        };
        self.tell(Request::SetProfile(profile))
    }

    /// Makes a group with the user as its owner; it shows in the next `chats`.
    #[wasm_bindgen(js_name = newGroup)]
    pub fn new_group(&self, name: &str) -> Result<(), JsValue> {
        self.tell(Request::NewGroup(name.to_owned()))
    }

    #[wasm_bindgen(js_name = addMember)]
    pub fn add_member(&self, group: f64, friend: f64) -> Result<(), JsValue> {
        let (group, friend) = (id(group)?, id(friend)?);
        self.tell(Request::AddMember { group, friend })
    }

    #[wasm_bindgen(js_name = joinGroup)]
    pub fn join_group(&self, group: f64) -> Result<(), JsValue> {
        self.tell(Request::JoinGroup(id(group)?))
    }

    #[wasm_bindgen(js_name = leaveGroup)]
    pub fn leave_group(&self, group: f64) -> Result<(), JsValue> {
        self.tell(Request::LeaveGroup(id(group)?))
    }

    /// The user first, then everyone still in the group.
    pub fn members(&self, group: f64) -> Result<js_sys::Promise, JsValue> {
        let group = id(group)?;
        Ok(self.ask(Want::Members(group), Request::Members(group)))
    }
}
