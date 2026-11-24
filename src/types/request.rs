//! What you ask the client to do.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::types::kind::FileKind;
use crate::types::model::bytes;
use crate::types::model::{
    CallRequest, ChatPrefs, ChatProfile, ChatRef, GroupPrefs, LinkInfo, MemberRole, Picture,
    StreamRequest,
};

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
    /// A message goes to another chat as the user's own, marked forwarded
    /// (`APIForwardChatItems`); a file goes again only when it is here.
    Forward {
        from: ChatRef,
        item: i64,
        to: ChatRef,
    },
    /// A live message: the friend or group sees it as it is written (`APISendMessages` live);
    /// `UpdateLive` sends the rest.
    SendLive {
        chat: ChatRef,
        text: String,
    },
    /// The live message's text so far; `done` finishes it (`APIUpdateChatItem` live).
    UpdateLive {
        chat: ChatRef,
        item: i64,
        text: String,
        done: bool,
    },
    /// A text with a link, and the link's preview (`MCLink`); the page makes the preview.
    SendLink {
        chat: ChatRef,
        text: String,
        preview: LinkInfo,
        reply_to: Option<i64>,
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
    /// An admin changes a member's role (`APIMembersRole`); an invited member who has not
    /// joined yet is sent the invitation again with it.
    SetMemberRole {
        group: i64,
        member: i64,
        role: MemberRole,
    },
    /// A moderator blocks a member's messages for everyone (`APIBlockMembersForAll`).
    BlockMemberForAll {
        group: i64,
        member: i64,
        on: bool,
    },
    /// An admin removes a member, and with `messages` marks what they wrote deleted for all
    /// (`APIRemoveMembers`).
    RemoveMember {
        group: i64,
        member: i64,
        messages: bool,
    },
    /// The group is deleted here; an owner deletes it for every member too (`APIDeleteChat`).
    DeleteGroup(i64),
    /// An owner changes the group's name and description (`APIUpdateGroupProfile`).
    SetGroupProfile {
        group: i64,
        name: String,
        description: Option<String>,
    },
    /// An owner changes the group's preferences (`APIUpdateGroupProfile`).
    SetGroupPrefs {
        group: i64,
        prefs: GroupPrefs,
    },
    /// Asks a group member to talk directly: a one-time link goes over the group connection,
    /// with `text` if given (`APICreateMemberContact`, `APISendMemberContactInvitation`). The
    /// member shows as a friend, connecting, until they accept.
    ContactMember {
        group: i64,
        member: i64,
        text: Option<String>,
    },
    /// "Fix connection": a new ratchet with a friend, or with `member` of a group
    /// (`APISyncContactRatchet`, `APISyncGroupMemberRatchet`). Without `force` only a
    /// connection that needs one is fixed; the engine starts that one itself anyway.
    SyncRatchet {
        chat: ChatRef,
        member: Option<i64>,
        force: bool,
    },
    /// A moderator deletes a member's message for everyone (`APIDeleteMemberChatItem`).
    Moderate {
        group: i64,
        item: i64,
    },
    /// Deletes a message here only, with its file (`APIDeleteChatItem` `CIDMInternal`).
    DeleteForMe {
        chat: ChatRef,
        item: i64,
    },
    /// Deletes every message of a conversation here, with their files (`APIClearChat`).
    ClearChat(ChatRef),
    /// The conversation is read up to and including `item` (`APIChatItemsRead`).
    MarkReadUpTo {
        chat: ChatRef,
        item: i64,
    },
    /// Marks a conversation unread, or takes the mark off (`APIChatUnread`).
    SetUnreadMark {
        chat: ChatRef,
        on: bool,
    },
    /// The user's own name for a friend or a group, never sent (`APISetContactAlias`,
    /// `APISetGroupAlias`); empty takes it off.
    SetAlias {
        chat: ChatRef,
        alias: String,
    },
    /// Messages older than `ttl` seconds are deleted here (`APISetChatItemTTL`,
    /// `APISetChatTTL`). `chat: None` sets the user's own, `None` or 0 keeping them; for one
    /// chat `None` follows the user's own and `Some(0)` keeps them.
    SetMessageTtl {
        chat: Option<ChatRef>,
        ttl: Option<i64>,
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
