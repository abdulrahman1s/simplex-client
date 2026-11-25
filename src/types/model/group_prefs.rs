//! A group's preferences as an owner sets them (Types/Preferences.hs `GroupPreferences`).

use serde::{Deserialize, Serialize};

use super::MemberRole;

/// One group feature: on or off, and for some the lowest role that may use it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupPref {
    pub on: bool,
    /// Who may use it, for direct messages, voice, files and SimpleX links; everyone when missing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<MemberRole>,
}

/// What to change of a group's preferences; one left out stays as the group has it.
/// Disappearing messages are set with `Request::SetTimer`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupPrefs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direct_messages: Option<GroupPref>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_delete: Option<GroupPref>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reactions: Option<GroupPref>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<GroupPref>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<GroupPref>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub simplex_links: Option<GroupPref>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reports: Option<GroupPref>,
    /// Recent messages sent to members who join.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<GroupPref>,
}
