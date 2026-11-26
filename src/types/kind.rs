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
    Timer,
    Prefs,
    /// An admin's or owner's change to a group, or deleting one.
    GroupAdmin,
    /// What the user keeps for themselves: deletions here, read marks, aliases, message TTLs.
    Local,
    Block,
    Code,
    Members,
    File,
    GroupLink,
    Call,
    /// `TestChatRelay`.
    RelayTest,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileKind {
    Image,
    Gif,
    Video,
    /// A voice message: its `duration` in seconds goes with it.
    Voice,
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
    /// Not one of the reactions there are, already given or taken back, or a fourth one.
    #[serde(rename = "SX-CHT-018")]
    ReactionNotAllowed,
    /// A file over the 1 GiB the official client sends, or over the in-memory limit.
    #[serde(rename = "SX-CHT-019")]
    FileTooLarge,
    /// A message, caption or bio longer than a message may carry.
    #[serde(rename = "SX-CHT-020")]
    TextTooLong,
    /// More than three members mentioned in one message.
    #[serde(rename = "SX-CHT-021")]
    TooManyMentions,
    /// A display or group name the official client would change.
    #[serde(rename = "SX-CHT-022")]
    InvalidName,
    /// The user's group role, or the chat's preferences, do not allow it; or its time has passed.
    #[serde(rename = "SX-CHT-023")]
    NotAllowed,
    /// A profile or group picture larger than a profile carries.
    #[serde(rename = "SX-CHT-024")]
    PictureTooLarge,
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
            Self::ReactionNotAllowed => "SX-CHT-018",
            Self::FileTooLarge => "SX-CHT-019",
            Self::TextTooLong => "SX-CHT-020",
            Self::TooManyMentions => "SX-CHT-021",
            Self::InvalidName => "SX-CHT-022",
            Self::NotAllowed => "SX-CHT-023",
            Self::PictureTooLarge => "SX-CHT-024",
        }
    }

    /// The code as JS sees it in a rejection's `code`: `reaction_not_allowed`, `file_too_large`, …
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::NoConnection => "no_connection",
            Self::BadLink => "bad_link",
            Self::RequestFailed => "request_failed",
            Self::CallFailed => "call_failed",
            Self::StreamFailed => "stream_failed",
            Self::ReactionNotAllowed => "reaction_not_allowed",
            Self::FileTooLarge => "file_too_large",
            Self::TextTooLong => "text_too_long",
            Self::TooManyMentions => "too_many_mentions",
            Self::InvalidName => "invalid_name",
            Self::NotAllowed => "not_allowed",
            Self::PictureTooLarge => "picture_too_large",
        }
    }

    /// A limit or rule of the official client (`simplex_client::limits`) refused the request,
    /// before anything was sent.
    #[must_use]
    pub const fn is_limit(self) -> bool {
        match self {
            Self::ReactionNotAllowed
            | Self::FileTooLarge
            | Self::TextTooLong
            | Self::TooManyMentions
            | Self::InvalidName
            | Self::NotAllowed
            | Self::PictureTooLarge => true,
            Self::NoConnection
            | Self::BadLink
            | Self::RequestFailed
            | Self::CallFailed
            | Self::StreamFailed => false,
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
