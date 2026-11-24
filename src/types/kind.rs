//! Which request failed and why, and what a sent file is.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RequestKind {
    Start,
    SetProfile,
    CreateInvite,
    Connect,
    Answer,
    Address,
    Send,
    MarkRead,
    History,
    Refresh,
    Group,
    RemoveFriend,
    SendFile,
    Edit,
    Delete,
    React,
    Mute,
    Block,
    Code,
    Members,
    File,
    GroupLink,
    Call,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    Image,
    Gif,
    Video,
    File,
}

/// Why a request failed: the stable codes [`Event::Failed`](crate::types::Event::Failed) carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ErrorCode {
    /// A relay did not answer (network, timeout, host).
    #[serde(rename = "SX-CHT-004")]
    NoConnection,
    /// The link was refused or could not be used.
    #[serde(rename = "SX-CHT-005")]
    BadLink,
    #[serde(rename = "SX-CHT-006")]
    RequestFailed,
    #[serde(rename = "SX-CHT-012")]
    CallFailed,
    #[serde(rename = "SX-CHT-017")]
    StreamFailed,
}

impl ErrorCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoConnection => "SX-CHT-004",
            Self::BadLink => "SX-CHT-005",
            Self::RequestFailed => "SX-CHT-006",
            Self::CallFailed => "SX-CHT-012",
            Self::StreamFailed => "SX-CHT-017",
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
