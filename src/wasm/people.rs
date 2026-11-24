//! The user's links, profile, friends and groups from JS.
use wasm_bindgen::prelude::*;

use super::client::Client;
use super::convert::{error, num, settled};
use super::objects::{Friend, Group};
use super::waiters::Want;
use crate::types::Request;

#[wasm_bindgen]
impl Client {
    /// A one-time invitation link for one person.
    #[wasm_bindgen(js_name = createInvite, unchecked_return_type = "Promise<string>")]
    pub fn create_invite(&self) -> js_sys::Promise {
        self.shared.ask(Want::Invite, Request::CreateInvite)
    }

    /// The user's long-lived address, made on first use.
    #[wasm_bindgen(js_name = createAddress, unchecked_return_type = "Promise<string>")]
    pub fn create_address(&self) -> js_sys::Promise {
        self.shared.ask(Want::Address, Request::CreateAddress)
    }

    #[wasm_bindgen(js_name = deleteAddress, unchecked_return_type = "Promise<void>")]
    pub fn delete_address(&self) -> js_sys::Promise {
        self.shared.queue(Request::DeleteAddress)
    }

    /// Connects through an invitation, an address or a group link.
    #[wasm_bindgen(unchecked_return_type = "Promise<void>")]
    pub fn connect(&self, link: &str) -> js_sys::Promise {
        self.shared.queue(Request::Connect(link.trim().to_owned()))
    }

    /// The user's chat preferences, sent to every friend they change: `{ voice: "no", … }`.
    #[wasm_bindgen(js_name = setPrefs, unchecked_return_type = "Promise<void>")]
    pub fn set_prefs(&self, prefs: JsValue) -> js_sys::Promise {
        super::objects::timer::set_prefs(&self.shared, None, &prefs)
    }

    /// Messages older than `seconds` are deleted here in every chat; `null` or `0` keeps them.
    #[wasm_bindgen(js_name = setMessageTtl, unchecked_return_type = "Promise<void>")]
    pub fn set_message_ttl(&self, seconds: Option<f64>) -> js_sys::Promise {
        super::objects::local::set_ttl(&self.shared, None, seconds)
    }

    #[wasm_bindgen(js_name = setName, unchecked_return_type = "Promise<void>")]
    pub fn set_name(&self, name: &str) -> js_sys::Promise {
        let profile = {
            let mut core = self.shared.core.borrow_mut();
            let p = core.as_mut().and_then(|c| c.profile.as_mut());
            p.map(|p| {
                name.clone_into(&mut p.name);
                p.clone()
            })
        };
        let Some(profile) = profile else {
            return settled(Err(error("closed", "the client is closed")));
        };
        self.shared.queue(Request::SetProfile(profile))
    }

    /// Makes a group with the user as its owner; resolves once the chat list shows it.
    #[wasm_bindgen(js_name = createGroup, unchecked_return_type = "Promise<Group>")]
    pub fn create_group(&self, name: &str) -> js_sys::Promise {
        self.shared
            .ask(Want::NewGroup, Request::NewGroup(name.to_owned()))
    }

    /// Friends from the last chat list, by id, connected or still connecting. A new `Map`
    /// on each read.
    #[wasm_bindgen(getter, unchecked_return_type = "Map<number, Friend>")]
    pub fn friends(&self) -> js_sys::Map {
        let list: Vec<_> = {
            let core = self.shared.core.borrow();
            let friends = core.as_ref().map(|c| c.roster.friends.values());
            friends.into_iter().flatten().cloned().collect()
        };
        let map = js_sys::Map::new();
        for f in list {
            let id = JsValue::from(num(f.id));
            map.set(
                &id,
                &Friend::new(std::rc::Rc::clone(&self.shared), f).into(),
            );
        }
        map
    }

    /// Groups from the last chat list, by id, left ones included. A new `Map` on each read.
    #[wasm_bindgen(getter, unchecked_return_type = "Map<number, Group>")]
    pub fn groups(&self) -> js_sys::Map {
        let list: Vec<_> = {
            let core = self.shared.core.borrow();
            let groups = core.as_ref().map(|c| c.roster.groups.values());
            groups.into_iter().flatten().cloned().collect()
        };
        let map = js_sys::Map::new();
        for g in list {
            let id = JsValue::from(num(g.id));
            map.set(&id, &Group::new(std::rc::Rc::clone(&self.shared), g).into());
        }
        map
    }
}
