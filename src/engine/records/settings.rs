//! The engine's own settings record.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettingsRecord {
    /// `sendRcptsContacts`, on for new users (Store/Profiles.hs).
    pub send_receipts: bool,
    /// Seconds after which messages are deleted here (`APISetChatItemTTL`); 0 keeps them.
    #[serde(default)]
    pub item_ttl: i64,
}

impl Default for SettingsRecord {
    fn default() -> Self {
        Self {
            send_receipts: true,
            item_ttl: 0,
        }
    }
}
