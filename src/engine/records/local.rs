//! What the user keeps about a conversation for themselves alone, never sent.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatLocal {
    /// The user's own name for the friend or group (`localAlias`); empty when none.
    #[serde(default)]
    pub alias: String,
    /// Marked unread by the user (`unreadChat`), whatever its messages say.
    #[serde(default)]
    pub marked_unread: bool,
    /// Seconds after which its messages are deleted here (`chatItemTTL`): `None` follows the
    /// user's own setting, `Some(0)` keeps them.
    #[serde(default)]
    pub item_ttl: Option<i64>,
}
