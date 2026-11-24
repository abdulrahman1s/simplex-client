//! Chat preferences: what the user allows each friend, and what is on between them (Types/Preferences.hs `Preferences`, `ContactUserPreference`).

use serde::{Deserialize, Serialize};

/// One feature of a direct chat (Types/Preferences.hs `ChatFeature`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChatFeature {
    /// Disappearing messages; their timer is set with `Request::SetTimer`.
    TimedMessages,
    /// Deleting a message for good rather than marking it deleted.
    FullDelete,
    Reactions,
    Voice,
    Files,
    Calls,
}

impl ChatFeature {
    pub const ALL: [Self; 6] = [
        Self::TimedMessages,
        Self::FullDelete,
        Self::Reactions,
        Self::Voice,
        Self::Files,
        Self::Calls,
    ];
}

/// How much one side allows a feature (Types/Preferences.hs `FeatureAllowed`): `Always` offers
/// it even to a friend who said no, for the user's own messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Allow {
    Always,
    Yes,
    No,
}

/// The user's preferences, or their overrides for one friend: a missing one falls back to the
/// user's own, then to the default (disappearing messages, reactions, voice and calls yes;
/// files always; full delete no).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatPrefs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timed_messages: Option<Allow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub full_delete: Option<Allow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reactions: Option<Allow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub voice: Option<Allow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<Allow>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calls: Option<Allow>,
}

/// Whether a feature is on for the user's messages and for the friend's (`PrefEnabled`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefEnabled {
    pub for_user: bool,
    pub for_contact: bool,
}
