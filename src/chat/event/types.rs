//! `ChatEvent`: one variant per event with its params (Protocol.hs `ChatMsgEvent`).
use crate::chat::call::{CallAnswer, CallExtraInfo, CallInvitation, CallOffer};
use crate::chat::channel::{
    GroupRelayInvitation, InlineFileInvitation, RelayCapabilities, RelayRejectionReason,
};
use crate::chat::container::{MsgContainer, MsgMentions, MsgScope};
use crate::chat::content::MsgContent;
use crate::chat::file::{FileDescr, FileInvitation};
use crate::chat::forward::GrpMsgForward;
use crate::chat::group::{
    GroupAcceptance, GroupInvitation, GroupLinkInvitation, GroupLinkRejection, GroupMemberRole,
    IntroInvitation, MemberInfo, MemberRestrictions,
};
use crate::chat::group_prefs::GroupPreferences;
use crate::chat::group_profile::GroupProfile;
use crate::chat::ids::{CallId, MemberId, Probe, ProbeHash, SharedMsgId, XContactId};
use crate::chat::message::ChatMessage;
use crate::chat::profile::Profile;
use crate::chat::reaction::MsgReaction;
use serde_json::{Map, Value};

/// The inline file chunk of an `F` element: a chunk number and bytes, or a cancel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FileChunk {
    Chunk { chunk_no: u8, bytes: Vec<u8> },
    Cancel,
}

/// Opaque strings (`conn_req`, member keys, links, times) are kept as sent.
#[derive(Clone, Debug, PartialEq)]
pub enum ChatEvent {
    MsgNew(MsgContainer),
    MsgFileDescr {
        msg_id: SharedMsgId,
        file_descr: FileDescr,
        file_expires: Option<String>,
        file_badge: Option<Value>,
    },
    MsgUpdate {
        msg_id: SharedMsgId,
        content: MsgContent,
        mentions: MsgMentions,
        ttl: Option<i64>,
        live: Option<bool>,
        scope: Option<MsgScope>,
        as_group: Option<bool>,
    },
    MsgDel {
        msg_id: SharedMsgId,
        member_id: Option<MemberId>,
        scope: Option<MsgScope>,
        only_history: bool,
    },
    MsgDeleted,
    MsgReact {
        msg_id: SharedMsgId,
        member_id: Option<MemberId>,
        scope: Option<MsgScope>,
        reaction: MsgReaction,
        add: bool,
    },
    File(FileInvitation),
    FileAcpt {
        file_name: String,
    },
    FileAcptInv {
        msg_id: SharedMsgId,
        file_conn_req: Option<String>,
        file_name: String,
    },
    FileCancel {
        msg_id: SharedMsgId,
    },
    Info {
        profile: Profile,
        member_key: Option<String>,
    },
    Contact {
        profile: Profile,
        member_key: Option<String>,
        contact_req_id: Option<XContactId>,
        welcome_msg_id: Option<SharedMsgId>,
        request_msg: Option<(SharedMsgId, MsgContent)>,
    },
    Member {
        profile: Profile,
        new_member_id: MemberId,
        new_member_key: String,
        via_relay: Option<MemberId>,
    },
    DirectDel,
    GrpInv(GroupInvitation),
    GrpAcpt {
        member_id: MemberId,
        member_key: Option<String>,
        profile: Option<Profile>,
    },
    GrpLinkInv(GroupLinkInvitation),
    GrpLinkReject(GroupLinkRejection),
    GrpLinkMem {
        profile: Profile,
        member_key: Option<String>,
    },
    GrpLinkAcpt {
        acceptance: GroupAcceptance,
        role: GroupMemberRole,
        member_id: MemberId,
    },
    /// An owner asks a relay to serve its channel (signed conn info).
    GrpRelayInv(GroupRelayInvitation),
    /// The relay's answer: its link for the channel.
    GrpRelayAcpt {
        relay_link: String,
        relay_cap: RelayCapabilities,
    },
    /// A relay test: the challenge, and the relay's signature of it in the answer.
    GrpRelayTest {
        challenge: Vec<u8>,
        signature: Option<Vec<u8>>,
    },
    GrpRelayNew {
        relay_link: String,
    },
    GrpRelayReject {
        reason: RelayRejectionReason,
    },
    GrpRelayCap(RelayCapabilities),
    GrpMemNew {
        member_info: MemberInfo,
        scope: Option<MsgScope>,
    },
    GrpMemIntro {
        member_info: MemberInfo,
        restrictions: Option<MemberRestrictions>,
    },
    GrpMemInv {
        member_id: MemberId,
        intro: IntroInvitation,
    },
    GrpMemFwd {
        member_info: MemberInfo,
        intro: IntroInvitation,
    },
    GrpMemInfo {
        member_id: MemberId,
        profile: Profile,
    },
    GrpMemRole {
        member_id: MemberId,
        role: GroupMemberRole,
        member_key: Option<String>,
        roster_version: Option<u64>,
    },
    GrpMemRestrict {
        member_id: MemberId,
        restrictions: MemberRestrictions,
    },
    GrpMemCon {
        member_id: MemberId,
    },
    GrpMemConAll {
        member_id: MemberId,
    },
    GrpMemDel {
        member_id: MemberId,
        messages: bool,
        roster_version: Option<u64>,
    },
    GrpLeave,
    GrpDel,
    GrpInfo(GroupProfile),
    GrpPrefs(GroupPreferences),
    GrpDirectInv {
        conn_req: String,
        content: Option<MsgContent>,
        scope: Option<MsgScope>,
    },
    /// The owner-signed roster header; the blob follows as `F` chunks under this message's id.
    GrpRoster {
        version: i64,
        file_inv: InlineFileInvitation,
    },
    GrpRosterAck {
        version: i64,
        error: Option<String>,
    },
    GrpRosterRequest {
        version: Option<i64>,
    },
    GrpMsgForward {
        fwd: GrpMsgForward,
        msg: Box<ChatMessage>,
    },
    InfoProbe(Probe),
    InfoProbeCheck(ProbeHash),
    InfoProbeOk(Probe),
    CallInv {
        call_id: CallId,
        invitation: CallInvitation,
    },
    CallOffer {
        call_id: CallId,
        offer: CallOffer,
    },
    CallAnswer {
        call_id: CallId,
        answer: CallAnswer,
    },
    CallExtra {
        call_id: CallId,
        extra: CallExtraInfo,
    },
    CallEnd {
        call_id: CallId,
    },
    Ok,
    /// The binary `F` element of the legacy inline file protocol.
    FileChunk {
        msg_id: SharedMsgId,
        chunk: FileChunk,
    },
    /// Names 7.1.4 does not know.
    Unknown {
        event: String,
        params: Map<String, Value>,
    },
}
