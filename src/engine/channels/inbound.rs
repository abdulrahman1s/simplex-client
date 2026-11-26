//! What reaches a channel member from a relay: posts forwarded as the channel or from a named member, checked by the channel's signature rules (Subscriber.hs `xGrpMsgForward`, `processForwardedMsg`, `FwdChannel`).

use crate::chat::element::ParsedMsg;
use crate::chat::forward::FwdSender;
use crate::chat::group::{GroupMemberRole, MemberInfo};
use crate::chat::{ChatEvent, ChatMessage};
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::groups::GroupCtx;
use crate::engine::groups::forward::forward_time;
use crate::engine::machine::Engine;
use crate::engine::records::{MemberCategory, MemberStatus};

fn dropped(why: &str) -> crate::engine::error::EngineError {
    fail(Code::RequestFailed, why.to_owned())
}

/// The content events a channel carries as posts.
fn is_content(e: &ChatEvent) -> bool {
    matches!(
        e,
        ChatEvent::MsgNew(_) | ChatEvent::MsgUpdate { .. } | ChatEvent::MsgDel { .. }
    )
}

fn as_group(e: &ChatEvent) -> bool {
    if let ChatEvent::MsgNew(c) = e {
        return c.as_group == Some(true);
    }
    matches!(
        e,
        ChatEvent::MsgUpdate {
            as_group: Some(true),
            ..
        }
    )
}

impl<A: AgentPort> Engine<A> {
    /// `isMemberGrpFwdRelay`: in a channel only a relay passes anything on.
    pub(crate) fn on_channel_element(
        &mut self,
        group: i64,
        sender: i64,
        p: ParsedMsg,
    ) -> Result<()> {
        if self.member(group, sender)?.role != GroupMemberRole::Relay {
            return Err(dropped("a channel message not from a relay"));
        }
        let Some(fwd) = &p.forward else {
            return self.on_relay_event(group, sender, p);
        };
        let at = forward_time(fwd).unwrap_or_else(|| self.clock.wall_ms());
        let FwdSender::Member {
            member_id,
            member_name,
        } = &fwd.sender
        else {
            // `FwdChannel`: unsigned, from nobody named, already checked by the relay.
            if !is_content(&p.message.event) {
                return Err(dropped("x.grp.msg.forward: event requires author"));
            }
            return self.on_channel_post(group, p.message, at, false);
        };
        if *member_id == self.group(group)?.my_member_id {
            return Ok(());
        }
        let author = match self.member_by_id(group, member_id).map(|m| m.id) {
            Some(a) => a,
            None if matches!(p.message.event, ChatEvent::GrpLeave) => return Ok(()),
            None => self.unknown_channel_member(group, member_id, member_name)?,
        };
        if self.member(group, author)?.status.gone() {
            return Ok(());
        }
        let signed =
            self.check_channel_signature(group, author, p.signed.as_ref(), &p.message.event, at)?;
        let owner = self.member(group, author)?.role == GroupMemberRole::Owner;
        // An owner's signed post as the channel shows as the channel's; anyone else's does not.
        if as_group(&p.message.event) && owner {
            let verified = signed == Some(crate::engine::groups::verify::SigStatus::Verified);
            return self.on_channel_post(group, p.message, at, verified);
        }
        let ctx = GroupCtx {
            group,
            sender,
            author,
            forwarded: true,
            signed,
        };
        self.on_channel_member_event(ctx, p.message, at)
    }

    fn on_channel_member_event(&mut self, ctx: GroupCtx, msg: ChatMessage, at: i64) -> Result<()> {
        match msg.event {
            e @ (ChatEvent::MsgNew(_)
            | ChatEvent::MsgUpdate { .. }
            | ChatEvent::MsgDel { .. }
            | ChatEvent::MsgReact { .. }) => {
                let shared = msg.msg_id.map(|m| m.0);
                self.on_group_message_at(ctx, shared, e, at)
            }
            // Verified above; the blob carries only the roles this engine does not keep.
            ChatEvent::GrpRoster { .. } => Ok(()),
            ChatEvent::GrpRelayNew { relay_link } => self.on_relay_new(ctx, &relay_link),
            e @ (ChatEvent::Info { .. }
            | ChatEvent::GrpMemRole { .. }
            | ChatEvent::GrpMemRestrict { .. }
            | ChatEvent::GrpMemDel { .. }
            | ChatEvent::GrpLeave
            | ChatEvent::GrpDel
            | ChatEvent::GrpInfo(_)
            | ChatEvent::GrpPrefs(_)) => self.on_group_admin_event(ctx, e),
            ChatEvent::MsgFileDescr { .. }
            | ChatEvent::MsgDeleted
            | ChatEvent::File(_)
            | ChatEvent::FileAcpt { .. }
            | ChatEvent::FileAcptInv { .. }
            | ChatEvent::FileCancel { .. }
            | ChatEvent::Contact { .. }
            | ChatEvent::Member { .. }
            | ChatEvent::DirectDel
            | ChatEvent::GrpInv(_)
            | ChatEvent::GrpAcpt { .. }
            | ChatEvent::GrpLinkInv(_)
            | ChatEvent::GrpLinkReject(_)
            | ChatEvent::GrpLinkMem { .. }
            | ChatEvent::GrpLinkAcpt { .. }
            | ChatEvent::GrpRelayInv(_)
            | ChatEvent::GrpRelayAcpt { .. }
            | ChatEvent::GrpRelayTest { .. }
            | ChatEvent::GrpRelayReject { .. }
            | ChatEvent::GrpRelayCap(_)
            | ChatEvent::GrpMemNew { .. }
            | ChatEvent::GrpMemIntro { .. }
            | ChatEvent::GrpMemInv { .. }
            | ChatEvent::GrpMemFwd { .. }
            | ChatEvent::GrpMemInfo { .. }
            | ChatEvent::GrpMemCon { .. }
            | ChatEvent::GrpMemConAll { .. }
            | ChatEvent::GrpDirectInv { .. }
            | ChatEvent::GrpRosterAck { .. }
            | ChatEvent::GrpRosterRequest { .. }
            | ChatEvent::GrpMsgForward { .. }
            | ChatEvent::InfoProbe(_)
            | ChatEvent::InfoProbeCheck(_)
            | ChatEvent::InfoProbeOk(_)
            | ChatEvent::CallInv { .. }
            | ChatEvent::CallOffer { .. }
            | ChatEvent::CallAnswer { .. }
            | ChatEvent::CallExtra { .. }
            | ChatEvent::CallEnd { .. }
            | ChatEvent::Ok
            | ChatEvent::FileChunk { .. }
            | ChatEvent::Unknown { .. } => {
                Err(dropped("x.grp.msg.forward: unsupported forwarded event"))
            }
        }
    }

    /// `getCreateUnknownGMByMemberId` with the subscriber role (`channelSubscriberRole`).
    fn unknown_channel_member(
        &mut self,
        group: i64,
        member_id: &crate::chat::ids::MemberId,
        name: &str,
    ) -> Result<i64> {
        let info = MemberInfo {
            member_id: member_id.clone(),
            member_role: GroupMemberRole::Observer,
            v: None,
            profile: crate::chat::profile::Profile::new(name, ""),
            member_key: None,
        };
        Ok(self
            .new_member(
                group,
                &info,
                MemberCategory::Post,
                MemberStatus::Unknown,
                None,
            )?
            .id)
    }
}
