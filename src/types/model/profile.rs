//! The user's profile and how people are around.

use serde::{Deserialize, Serialize};

use super::{ChatPrefs, Picture};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatProfile {
    pub name: String,
    pub about: String,
    pub paper: Option<u8>,
    pub status: MyStatus,
    pub picture: Option<Picture>,
    /// The user's chat preferences, as every friend is sent them unless overridden for one.
    /// Read only here: `Request::SetPrefs` changes them, `SetProfile` leaves them.
    #[serde(default)]
    pub prefs: ChatPrefs,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MyStatus {
    #[default]
    Online,
    Away,
    Busy,
    Invisible,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Presence {
    Online,
    Away,
    Busy,
    #[default]
    Offline,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Playing {
    pub title: String,
    pub minutes: u32,
    pub live: bool,
}
