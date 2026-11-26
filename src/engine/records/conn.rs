//! What an agent connection is to the engine: an invitation not answered yet, a friend's, a group member's, or a group link.

use serde::{Deserialize, Serialize};

use crate::engine::agent_port::ConnId;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnRecord {
    pub conn: ConnId,
    pub kind: ConnKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum ConnKind {
    /// Made by `CreateInvite`; the links arrive with the agent's `Invitation`.
    Invitation {
        full: Option<String>,
        short: Option<String>,
        /// Milliseconds since the epoch, for evicting the oldest.
        created: i64,
        /// Made incognito: the random name the user goes by with whoever answers.
        #[serde(default)]
        incognito: Option<String>,
    },
    Friend {
        id: i64,
    },
    /// A group connection with a member, from its invitation (made or joined) on. It never
    /// shows as an invitation, takes no part in `inviting`, and is not evicted with them.
    Member {
        group: i64,
        member: i64,
    },
    /// The contact address of a group's link.
    GroupLink {
        group: i64,
    },
    /// A link being read without joining it (`lookup_link`); gone with the answer.
    Lookup(super::Lookup),
    /// A chat relay's test connection; gone with its answer or its deadline.
    RelayTest(super::RelayTestRecord),
}

impl ConnKind {
    /// The relay test this connection is for, if it is one's.
    #[must_use]
    pub fn relay_test(&self) -> Option<&super::RelayTestRecord> {
        match self {
            Self::RelayTest(t) => Some(t),
            Self::Invitation { .. }
            | Self::Friend { .. }
            | Self::Member { .. }
            | Self::GroupLink { .. }
            | Self::Lookup(_) => None,
        }
    }
}
