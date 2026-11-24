//! Friends and requests to become one.

use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

use super::{ChatFeature, ChatPrefs, ChatRef, Picture, Playing, PrefEnabled, Presence};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Friend {
    pub id: i64,
    pub name: String,
    pub ready: bool,
    pub presence: Presence,
    pub playing: Option<Playing>,
    /// Milliseconds since the epoch.
    pub last_seen: Option<i64>,
    pub about: String,
    pub paper: Option<u8>,
    /// Milliseconds since the epoch.
    pub since: Option<i64>,
    pub verified: bool,
    pub blocked: bool,
    pub muted: bool,
    pub seen: Vec<String>,
    pub picture: Option<Picture>,
    /// Seconds new messages last, when disappearing messages are on for both sides.
    #[serde(default)]
    pub timer: Option<i64>,
    /// The user's preferences for this friend, over their own (`Request::SetPrefs`).
    #[serde(default)]
    pub prefs: ChatPrefs,
    /// What is on between them, feature by feature.
    #[serde(default)]
    pub features: BTreeMap<ChatFeature, PrefEnabled>,
    /// The user's own name for them, never sent; empty when none.
    #[serde(default)]
    pub alias: String,
    /// Seconds after which messages are deleted here: `None` follows the user's own setting,
    /// `Some(0)` keeps them.
    #[serde(default)]
    pub item_ttl: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FriendRequest {
    pub id: i64,
    pub name: String,
}

impl Friend {
    /// The conversation with this friend.
    #[must_use]
    pub const fn chat(&self) -> ChatRef {
        ChatRef::Direct(self.id)
    }
}
