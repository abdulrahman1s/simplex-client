//! What an admin, moderator or owner does from JS: a member's role, block and removal; the group's deletion, profile and preferences; moderating a message.

use wasm_bindgen::prelude::*;

use super::super::convert::{error, from_js, settled};
use super::{Group, Member, Message};
use crate::types::Request;
use crate::types::model::{ChatRef, GroupPrefs, MemberRole};

/// A member that came from a message, not the group's list, has no number to act on.
fn listed(m: &Member, r: impl FnOnce(i64, i64) -> Request) -> js_sys::Promise {
    let (shared, group, id) = m.parts();
    match id {
        Some(member) => shared.queue(r(group, member)),
        None => settled(Err(error(
            "bad_argument",
            "a member not from the group's list",
        ))),
    }
}

#[wasm_bindgen]
impl Member {
    /// `"observer"`, `"author"`, `"member"`, `"moderator"`, `"admin"` or `"owner"`; admins only.
    #[wasm_bindgen(js_name = setRole, unchecked_return_type = "Promise<void>")]
    pub fn set_role(&self, role: &str) -> js_sys::Promise {
        let Ok(role) = serde_json::from_value::<MemberRole>(role.into()) else {
            return settled(Err(error("bad_argument", format!("not a role: {role}"))));
        };
        listed(self, |group, member| Request::SetMemberRole {
            group,
            member,
            role,
        })
    }

    /// Hides or shows the member's messages for everyone; moderators only.
    #[wasm_bindgen(js_name = blockForAll, unchecked_return_type = "Promise<void>")]
    pub fn block_for_all(&self, on: bool) -> js_sys::Promise {
        listed(self, |group, member| Request::BlockMemberForAll {
            group,
            member,
            on,
        })
    }

    /// Asks the member to talk directly, with `text` if given; they show as a friend,
    /// connecting, until they accept.
    #[wasm_bindgen(unchecked_return_type = "Promise<void>")]
    pub fn contact(&self, text: Option<String>) -> js_sys::Promise {
        listed(self, |group, member| Request::ContactMember {
            group,
            member,
            text,
        })
    }

    /// Removes the member; with `messages`, what they wrote shows deleted for all. Admins only.
    #[wasm_bindgen(unchecked_return_type = "Promise<void>")]
    pub fn remove(&self, messages: Option<bool>) -> js_sys::Promise {
        let messages = messages.unwrap_or(false);
        listed(self, |group, member| Request::RemoveMember {
            group,
            member,
            messages,
        })
    }
}

#[wasm_bindgen]
impl Group {
    /// Deletes the group here; an owner deletes it for every member too.
    #[wasm_bindgen(unchecked_return_type = "Promise<void>")]
    pub fn delete(&self) -> js_sys::Promise {
        let ChatRef::Group(group) = self.chat() else {
            return settled(Ok(()));
        };
        self.shared().queue(Request::DeleteGroup(group))
    }

    /// An owner renames the group or changes its description (`null` for none).
    #[wasm_bindgen(js_name = setProfile, unchecked_return_type = "Promise<void>")]
    pub fn set_profile(&self, name: &str, description: Option<String>) -> js_sys::Promise {
        let ChatRef::Group(group) = self.chat() else {
            return settled(Ok(()));
        };
        self.shared().queue(Request::SetGroupProfile {
            group,
            name: name.to_owned(),
            description,
        })
    }

    /// An owner turns features on or off: `{ voice: { on: false }, files: { on: true, role: "member" } }`.
    #[wasm_bindgen(js_name = setPrefs, unchecked_return_type = "Promise<void>")]
    pub fn set_prefs(&self, prefs: JsValue) -> js_sys::Promise {
        let ChatRef::Group(group) = self.chat() else {
            return settled(Ok(()));
        };
        match serde_json::from_value::<GroupPrefs>(from_js(&prefs)) {
            Ok(prefs) => self.shared().queue(Request::SetGroupPrefs { group, prefs }),
            Err(e) => settled(Err(error(
                "bad_argument",
                format!("not group preferences: {e}"),
            ))),
        }
    }
}

#[wasm_bindgen]
impl Message {
    /// A moderator deletes a member's message in a group for everyone.
    #[wasm_bindgen(unchecked_return_type = "Promise<void>")]
    pub fn moderate(&self) -> js_sys::Promise {
        let info = self.info();
        let ChatRef::Group(group) = info.chat else {
            return settled(Err(error(
                "bad_argument",
                "only a group's message is moderated",
            )));
        };
        self.shared().queue(Request::Moderate {
            group,
            item: info.id,
        })
    }
}
