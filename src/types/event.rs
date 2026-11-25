//! What the client hears: replies to requests and everything that happens on its own.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::types::kind::{ErrorCode, FileKind, RequestKind};
use crate::types::model::{
    CallSignal, ChatFeatures, ChatGroup, ChatMessage, ChatPreview, ChatProfile, ChatRef, Friend,
    FriendRequest, GroupMember, SendStatus, StreamSignal,
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", content = "c", rename_all = "snake_case")]
pub enum Event {
    Features {
        features: ChatFeatures,
        /// The reactions it carries; `None` = any emoji.
        reactions: Option<Vec<String>>,
    },
    Ready(ChatProfile),
    Profile(ChatProfile),
    Chats {
        friends: Vec<Friend>,
        groups: Vec<ChatGroup>,
        requests: Vec<FriendRequest>,
        chats: Vec<ChatPreview>,
    },
    /// What changed in the chat list since it last went out, instead of a whole `Chats`, when
    /// `Config::roster_deltas` is on: each friend and group changed or new as it is now, the
    /// ids of those gone, and the requests when they changed. Previews (`Chats.chats`) come
    /// only with a whole list.
    ChatsChanged {
        friends: Vec<Friend>,
        groups: Vec<ChatGroup>,
        removed_friends: Vec<i64>,
        removed_groups: Vec<i64>,
        requests: Option<Vec<FriendRequest>>,
    },
    Invite(String),
    Address(Option<String>),
    Messages(Vec<ChatMessage>),
    Updated(Vec<ChatMessage>),
    History {
        tag: u64,
        chat: ChatRef,
        messages: Vec<ChatMessage>,
    },
    Code {
        friend: i64,
        code: String,
    },
    Verified {
        friend: i64,
        matched: bool,
    },
    Members {
        group: i64,
        members: Vec<GroupMember>,
    },
    /// Application data under a tag the client registered (`Config::data_tags`), sent with
    /// `Request::SendData` or anything else that sends that content type. `member` is the group
    /// member who sent it, `None` in a direct chat. It is not kept: a message the relay
    /// delivers again after a restart, before it was acknowledged, comes again.
    Data {
        chat: ChatRef,
        member: Option<i64>,
        tag: String,
        json: serde_json::Map<String, serde_json::Value>,
    },
    /// What became of the messages a `Request::Tagged` sent; `member` names the group member
    /// a `Sent` or `Failed` is for.
    SendStatus {
        tag: u64,
        member: Option<i64>,
        status: SendStatus,
    },
    /// Every message of the conversation was deleted here (`Request::ClearChat`).
    Cleared(ChatRef),
    Offline(bool),
    Preview {
        chat: ChatRef,
        last: Box<ChatMessage>,
    },
    OpenFile {
        path: PathBuf,
        kind: FileKind,
    },
    Located {
        tag: u64,
        path: Option<PathBuf>,
    },
    GroupLink {
        group: i64,
        link: Option<String>,
    },
    Call(CallSignal),
    Stream(StreamSignal),
    Failed {
        request: RequestKind,
        code: ErrorCode,
        /// The engine's own words, for the log.
        detail: String,
    },
    Exited {
        status: Option<i32>,
        started: bool,
    },
}
