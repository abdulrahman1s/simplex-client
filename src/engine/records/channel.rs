//! A channel's part of a group (Types.hs `GroupInfo.useRelays`, `GroupKeys.GKPublicGroup`; Operators.hs `GroupRelay`, `RelayStatus`).

use serde::{Deserialize, Serialize};

/// `RelayStatus` (Types/Shared.hs): where a relay stands with the user's channel. A subscriber
/// keeps none; it sees relays only as members.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelayStatus {
    New,
    Invited,
    Accepted,
    AcknowledgedRoster,
    Active,
    Inactive,
    Rejected,
}

/// One relay of an owner's channel (`GroupRelay`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelayRecord {
    /// The relay's member number in the group.
    pub member: i64,
    /// The relay's own address (`/r#…`) the user configured.
    pub address: String,
    pub name: String,
    pub status: RelayStatus,
    /// The relay's link for this channel, from its `x.grp.relay.acpt`.
    pub link: Option<String>,
    /// `RelayCapabilities.webDomain`.
    #[serde(default)]
    pub web_domain: Option<String>,
}

/// What makes a group a channel: its identity, the user's keys, and an owner's relays.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelRecord {
    /// `publicGroupId`: `sha256` of the root key.
    pub public_group_id: Vec<u8>,
    /// The channel's short link (`/c#…`).
    pub link: String,
    /// The root key, X.509 DER: what signs the link data and the owners.
    pub root_key: Vec<u8>,
    /// Owner only: the root private key, PKCS#8 DER.
    #[serde(default)]
    pub root_private: Option<Vec<u8>>,
    /// The user's member key pair's private half, PKCS#8 DER: what signs the user's messages.
    pub member_private: Vec<u8>,
    /// Owner only. Eviction: a relay's entry goes with its member.
    #[serde(default)]
    pub relays: Vec<RelayRecord>,
    /// The owner's roster version (`rosterVersion`), 0 from the start.
    #[serde(default)]
    pub roster_version: Option<i64>,
    /// The channel link's contact connection (owner only).
    #[serde(default)]
    pub link_conn: Option<crate::engine::agent_port::ConnId>,
    /// Subscribers in the link data (`publicMemberCount`), as last read or published.
    #[serde(default)]
    pub member_count: Option<i64>,
    /// Owner only: the relay links in the link data being stored now, active once it is.
    #[serde(default)]
    pub publishing: Vec<String>,
    /// Shared ids of application-data posts already told, so another relay's copy is not.
    /// Eviction: the oldest goes past [`SEEN_DATA_POSTS`].
    #[serde(default)]
    pub seen: std::collections::VecDeque<Vec<u8>>,
}

/// How many application-data posts a channel remembers having told (relays bring each once).
pub const SEEN_DATA_POSTS: usize = 256;

impl ChannelRecord {
    #[must_use]
    pub fn owner(&self) -> bool {
        self.root_private.is_some()
    }

    #[must_use]
    pub fn relay(&self, member: i64) -> Option<&RelayRecord> {
        self.relays.iter().find(|r| r.member == member)
    }

    pub fn relay_mut(&mut self, member: i64) -> Option<&mut RelayRecord> {
        self.relays.iter_mut().find(|r| r.member == member)
    }

    /// `false` when `shared` was told already; else it is remembered.
    pub fn first_sight(&mut self, shared: &[u8]) -> bool {
        if self.seen.iter().any(|s| s == shared) {
            return false;
        }
        self.seen.push_back(shared.to_vec());
        while self.seen.len() > SEEN_DATA_POSTS {
            self.seen.pop_front();
        }
        true
    }
}

/// Why a link is being read (chat's `CFGetRelayDataJoin`, `CFGetRelayDataAccept`, …).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Lookup {
    /// A channel's link, to join it.
    Channel { link: String },
    /// A subscriber's relay of `group`, to join the relay.
    RelayJoin { group: i64, member: i64 },
    /// An owner's relay address, to invite the relay.
    RelayAddress { group: i64, member: i64 },
    /// An owner's relay link from its acceptance, to check it names the relay.
    RelayAccept { group: i64, member: i64 },
}
