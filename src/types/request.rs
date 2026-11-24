//! What you ask the client to do.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::types::kind::FileKind;
use crate::types::model::bytes;
use crate::types::model::{CallRequest, ChatPrefs, ChatProfile, ChatRef, Picture, StreamRequest};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "t", content = "c", rename_all = "snake_case")]
pub enum Request {
    SetProfile(ChatProfile),
    SetPicture(Option<Picture>),
    SetGroupPicture {
        group: i64,
        picture: Option<Picture>,
    },
    CreateInvite,
    Connect(String),
    Accept(i64),
    Reject(i64),
    CreateAddress,
    DeleteAddress,
    Send {
        chat: ChatRef,
        text: String,
        reply_to: Option<i64>,
        /// Members named with @: name as written, member number.
        mentions: Vec<(String, i64)>,
    },
    SendFile {
        chat: ChatRef,
        path: PathBuf,
        kind: FileKind,
        #[serde(with = "bytes::opt", default)]
        preview: Option<Vec<u8>>,
        duration: Option<u32>,
        text: String,
    },
    ReceiveFile {
        chat: ChatRef,
        item: i64,
    },
    CancelFile {
        chat: ChatRef,
        item: i64,
    },
    OpenFile {
        chat: ChatRef,
        item: i64,
    },
    LocateFile {
        chat: ChatRef,
        item: i64,
        tag: u64,
    },
    CreateGroupLink(i64),
    DeleteGroupLink(i64),
    Edit {
        chat: ChatRef,
        item: i64,
        text: String,
    },
    Delete {
        chat: ChatRef,
        item: i64,
    },
    React {
        chat: ChatRef,
        item: i64,
        emoji: String,
        on: bool,
    },
    SetMuted {
        chat: ChatRef,
        on: bool,
    },
    /// Disappearing messages: new messages in the chat go `ttl` seconds after they are sent
    /// (the user's) or read (received ones); `None` turns it off. For a friend it is offered
    /// to them; for a group it is the group's preference, which only an owner sets.
    SetTimer {
        chat: ChatRef,
        ttl: Option<i64>,
    },
    /// Chat preferences: the user's own with `friend: None`, sent to every friend they change,
    /// or the user's overrides for one friend. A feature left out is not set at that level;
    /// disappearing messages keep their timer and their "allow" unless named.
    SetPrefs {
        friend: Option<i64>,
        prefs: ChatPrefs,
    },
    SetBlocked {
        friend: i64,
        on: bool,
    },
    SetMemberBlocked {
        group: i64,
        member: i64,
        on: bool,
    },
    ShowCode(i64),
    Verify {
        friend: i64,
        code: String,
    },
    Members(i64),
    MarkRead(ChatRef),
    History {
        chat: ChatRef,
        before: Option<i64>,
        count: u32,
        tag: u64,
    },
    Refresh,
    NewGroup(String),
    AddMember {
        group: i64,
        friend: i64,
    },
    JoinGroup(i64),
    LeaveGroup(i64),
    RemoveFriend(i64),
    Call(CallRequest),
    Stream(StreamRequest),
}
