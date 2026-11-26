//! Channels as the caller sees them: chat relays and where each stands (Operators.hs `UserChatRelay`, `GroupRelay`; Types/Shared.hs `RelayStatus`).

use serde::{Deserialize, Serialize};

/// A chat relay the caller supplies: its address (`/r#…`) and a name to show.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatRelay {
    pub address: String,
    pub name: String,
}

/// `RelayStatus`: an owner's relay, from invited to active (its link published).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelayStatus {
    #[default]
    New,
    Invited,
    Accepted,
    AcknowledgedRoster,
    Active,
    Inactive,
    Rejected,
}

/// One relay of a channel (`GroupRelay`). A subscriber knows no status, only whether the
/// relay is connected.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelRelay {
    /// The relay's member number.
    pub member: i64,
    pub name: String,
    /// The owner's view; `None` for a subscriber.
    pub status: Option<RelayStatus>,
    /// The relay's link for this channel.
    pub link: Option<String>,
    pub connected: bool,
    /// Whether this connection's ratchet uses the post-quantum KEM. Channels are made without
    /// it, as 7.1.4 makes them, so this is `false`; a caller may refuse a channel for it.
    pub pq: bool,
}

/// What makes a group a channel (`GroupInfo.useRelays`, `GroupType::channel`): posts go
/// through relays, which read and keep them. Nothing in a channel is end-to-end encrypted
/// between owner and subscribers.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelInfo {
    /// `useRelays`, always true here.
    pub use_relays: bool,
    /// The channel's link (`/c#…`).
    pub link: String,
    /// `publicGroupId`, padded base64url.
    pub public_group_id: String,
    pub owner: bool,
    pub relays: Vec<ChannelRelay>,
}

/// simplex-chat 7.1.4's preset chat relays (Operators/Presets.hs `simplexChatRelays`), run by
/// SimpleX Chat Ltd. Unlike the preset SMP relays they are never used unless the caller passes
/// them: SimpleX Chat's preset-server policy limits third-party apps to "user-to-user messaging
/// only or … chat bots sending messages requested by users", which a channel may not be.
const PRESET_CHAT_RELAYS: [(&str, &str); 3] = [
    (
        "SimpleX Chat Relay 1",
        "https://smp5.simplex.im/r#Fp5RWXkiRFg-hgcDwC2v-MWnPfvEf42RgCqREntW0mw",
    ),
    (
        "SimpleX Chat Relay 2",
        "https://smp6.simplex.im/r#_qlQfogHGDJ8MAF2wKmkglRBM-xHR142gDJstKiGRQQ",
    ),
    (
        "SimpleX Chat Relay 3",
        "https://smp4.simplex.im/r#yxNOMJcry5jMTRPEBVtGBATYaKeoRIsZRBPIDLx7x6M",
    ),
];

impl ChatRelay {
    /// simplex-chat 7.1.4's preset chat relays, run by SimpleX Chat Ltd, for a caller whose use
    /// SimpleX Chat's preset-server policy allows ("user-to-user messaging only or … chat bots
    /// sending messages requested by users"). Nothing uses them by default.
    #[must_use]
    pub fn presets() -> Vec<Self> {
        PRESET_CHAT_RELAYS
            .iter()
            .map(|(name, address)| Self {
                address: (*address).to_owned(),
                name: (*name).to_owned(),
            })
            .collect()
    }
}
