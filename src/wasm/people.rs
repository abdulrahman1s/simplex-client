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
    /// `{ incognito: true }`: whoever joins knows the user by a random name only.
    #[wasm_bindgen(js_name = createInvite, unchecked_return_type = "Promise<string>")]
    pub fn create_invite(
        &self,
        #[wasm_bindgen(unchecked_param_type = "{ incognito?: boolean }")] options: JsValue,
    ) -> js_sys::Promise {
        let request = if incognito(&options) {
            Request::CreateIncognitoInvite
        } else {
            Request::CreateInvite
        };
        self.shared.ask(Want::Invite, request)
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
    pub fn connect(
        &self,
        link: &str,
        #[wasm_bindgen(unchecked_param_type = "{ incognito?: boolean }")] options: JsValue,
    ) -> js_sys::Promise {
        let link = link.trim().to_owned();
        if incognito(&options) {
            return self.shared.queue(Request::ConnectIncognito(link));
        }
        self.shared.queue(Request::Connect(link))
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

    /// `{ autoAccept, autoReply }`: requests to the address are accepted as they come, and
    /// `autoReply` sent to each new friend once connected.
    #[wasm_bindgen(js_name = setAddressSettings, unchecked_return_type = "Promise<void>")]
    pub fn set_address_settings(
        &self,
        #[wasm_bindgen(
            unchecked_param_type = "{ autoAccept?: boolean, autoReply?: string | null }"
        )]
        settings: JsValue,
    ) -> js_sys::Promise {
        let json = super::convert::from_js(&settings);
        let auto_accept = json
            .get("autoAccept")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let auto_reply = json
            .get("autoReply")
            .and_then(|v| v.as_str())
            .map(str::to_owned);
        self.shared.queue(Request::SetAddressSettings {
            auto_accept,
            auto_reply,
        })
    }

    /// Shows the address in the profile every friend gets, or takes it out.
    #[wasm_bindgen(js_name = shareAddress, unchecked_return_type = "Promise<void>")]
    pub fn share_address(&self, on: bool) -> js_sys::Promise {
        self.shared.queue(Request::ShareAddress(on))
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

    /// Makes a channel owned by the user on the relays `setChatRelays` gave; resolves once the
    /// chat list shows it. Relays read and keep every post, and the channel's connections have
    /// no post-quantum KEM: nothing in it is end-to-end encrypted.
    #[wasm_bindgen(js_name = createChannel, unchecked_return_type = "Promise<Group>")]
    pub fn create_channel(&self, name: &str) -> js_sys::Promise {
        self.shared
            .ask(Want::NewGroup, Request::NewChannel(name.to_owned()))
    }

    /// The chat relays new channels are made on, `[{ address, name }]`; only these are used.
    #[wasm_bindgen(js_name = setChatRelays, unchecked_return_type = "Promise<void>")]
    pub fn set_chat_relays(
        &self,
        #[wasm_bindgen(unchecked_param_type = "{ address: string, name: string }[]")]
        relays: JsValue,
    ) -> js_sys::Promise {
        let Ok(relays) = serde_json::from_value(super::convert::from_js(&relays)) else {
            return settled(Err(error("bad_argument", "relays are [{ address, name }]")));
        };
        self.shared.queue(Request::SetChatRelays(relays))
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

/// `{ incognito: true }` in a call's options.
fn incognito(options: &JsValue) -> bool {
    let json = super::convert::from_js(options);
    json.get("incognito").and_then(serde_json::Value::as_bool) == Some(true)
}
