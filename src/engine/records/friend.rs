//! A friend (Types.hs `Contact`): their profile as last received and the local settings.

use serde::{Deserialize, Serialize};

use super::ChatLocal;
use crate::chat::preferences::Preferences;
use crate::chat::version::VersionRange;
use crate::engine::agent_port::ConnId;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FriendRecord {
    pub id: i64,
    pub conn: ConnId,
    pub name: String,
    pub about: String,
    /// Their profile's `image`, a `data:` URI as sent.
    pub image: Option<String>,
    pub ready: bool,
    /// They deleted the contact (`x.direct.del`): the conversation stays, nothing more is sent.
    pub gone: bool,
    /// Milliseconds since the epoch.
    pub since: i64,
    pub muted: bool,
    /// Local only (never sent): their messages are deleted on arrival.
    pub blocked: bool,
    /// Items up to this id are read.
    pub read_up_to: i64,
    pub unread: u32,
    /// The link joined, in its `simplex:` form, so pasting it again connects nothing.
    pub link: Option<String>,
    /// The links of our invitation they answered, so ours pasted back connects nothing.
    #[serde(default)]
    pub invited_with: Vec<String>,
    /// Their profile's chat preferences as last received; `None` reads as the defaults.
    #[serde(default)]
    pub prefs: Option<Preferences>,
    /// The security code they were verified with (Haskell `connectionCode`); cleared when it changes.
    #[serde(default)]
    pub verified: Option<String>,
    /// The `pqEncryption` messages to them go with: on for a connection made with PQ, which a
    /// record from before this field was always is; a member contact's turns on with their first
    /// message (Internal.hs `updatePeerChatVRange`).
    #[serde(default = "pq_on")]
    pub pq: bool,
    /// Their chat version range, from the last message that carried one (`peerChatVRange`).
    #[serde(default)]
    pub chat_v: Option<VersionRange>,
    /// The user's preferences for this friend over their own (`Contact.userPreferences`): what
    /// `SetPrefs` set, and the timer, set or taken from their profile (`engine::timed`).
    #[serde(default)]
    pub my_prefs: Option<Preferences>,
    #[serde(default)]
    pub local: ChatLocal,
    /// The address their profile shows, if any (`Profile.contactLink`).
    #[serde(default)]
    pub address: Option<String>,
}

const fn pq_on() -> bool {
    true
}

impl FriendRecord {
    #[must_use]
    pub fn new(id: i64, conn: ConnId, since: i64) -> Self {
        Self {
            id,
            conn,
            name: String::new(),
            about: String::new(),
            image: None,
            ready: false,
            gone: false,
            since,
            muted: false,
            blocked: false,
            read_up_to: 0,
            unread: 0,
            link: None,
            invited_with: Vec::new(),
            prefs: None,
            verified: None,
            pq: true,
            chat_v: None,
            my_prefs: None,
            local: ChatLocal::default(),
            address: None,
        }
    }

    /// Whether messages can be sent to them.
    #[must_use]
    pub fn reachable(&self) -> bool {
        self.ready && !self.gone
    }
}
