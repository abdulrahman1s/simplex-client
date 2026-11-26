//! The engine's own settings record.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsRecord {
    /// `sendRcptsContacts`, on for new users (Store/Profiles.hs).
    pub send_receipts: bool,
    /// Seconds after which messages are deleted here (`APISetChatItemTTL`); 0 keeps them.
    #[serde(default)]
    pub item_ttl: i64,
    /// The chat relays new channels are made on (`UserChatRelay`), only ever the caller's.
    /// Eviction: replaced whole by `SetChatRelays`.
    #[serde(default)]
    pub chat_relays: Vec<crate::types::model::ChatRelay>,
}

impl Default for SettingsRecord {
    fn default() -> Self {
        Self {
            send_receipts: true,
            item_ttl: 0,
            chat_relays: Vec::new(),
        }
    }
}
