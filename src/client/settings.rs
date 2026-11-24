//! The user's settings that friends see: timers, chat preferences, the address's.

use super::error::Result;
use super::handle::Client;
use crate::types::Request;
use crate::types::model::{ChatPrefs, ChatRef};

impl Client {
    /// Disappearing messages in `chat`: new ones last `ttl` seconds, or `None` to turn it off.
    pub fn set_timer(&self, chat: ChatRef, ttl: Option<i64>) -> Result<()> {
        self.request(Request::SetTimer { chat, ttl })
    }
    /// The user's chat preferences, sent to every friend they change (`Request::SetPrefs`).
    pub fn set_prefs(&self, prefs: ChatPrefs) -> Result<()> {
        self.request(Request::SetPrefs {
            friend: None,
            prefs,
        })
    }
    /// Requests to the address are accepted as they come, and `auto_reply` sent to each new
    /// friend once connected.
    pub fn set_address_settings(&self, auto_accept: bool, auto_reply: Option<&str>) -> Result<()> {
        self.request(Request::SetAddressSettings {
            auto_accept,
            auto_reply: auto_reply.map(str::to_owned),
        })
    }
    /// Shows the address in the profile every friend gets, or takes it out.
    pub fn share_address(&self, on: bool) -> Result<()> {
        self.request(Request::ShareAddress(on))
    }
}
