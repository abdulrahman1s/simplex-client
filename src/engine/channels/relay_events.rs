//! A relay's own messages in a channel: its profile, the owners it introduces, the roster blob, and for an owner the roster ack, the subscribers it announces and its capabilities (Subscriber.hs `processEvent` for relay members, `xGrpLinkMem`, `xGrpMemIntro`, `xGrpRosterAck`, `XGrpRelayCap`).

use crate::chat::ChatEvent;
use crate::chat::element::ParsedMsg;
use crate::chat::group::MemberInfo;
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::machine::Engine;
use crate::engine::records::{MemberCategory, MemberStatus};

fn dropped(why: &str) -> crate::engine::error::EngineError {
    fail(Code::RequestFailed, why.to_owned())
}

impl<A: AgentPort> Engine<A> {
    /// The relay's own messages: its profile, the owners it introduces, the roster blob, and
    /// for an owner the roster ack and the subscribers it announces.
    pub(super) fn on_relay_event(&mut self, group: i64, relay: i64, p: ParsedMsg) -> Result<()> {
        let now = self.clock.wall_ms();
        self.check_channel_signature(group, relay, p.signed.as_ref(), &p.message.event, now)?;
        self.refresh_member_range(group, relay, &p.message)?;
        match p.message.event {
            ChatEvent::GrpLinkMem { profile, .. } => {
                let mut m = self.member(group, relay)?;
                m.profile = profile;
                Ok(self.save_member(m)?)
            }
            ChatEvent::GrpMemIntro { member_info, .. } => self.on_relay_intro(group, member_info),
            ChatEvent::GrpMemNew { member_info, .. } => self.on_relay_intro(group, member_info),
            ChatEvent::GrpRosterAck { error, .. } => self.on_roster_ack(group, relay, error),
            ChatEvent::GrpRelayCap(cap) => {
                let mut g = self.group(group)?;
                if let Some(r) = g.channel.as_mut().and_then(|c| c.relay_mut(relay)) {
                    r.web_domain = cap.web_domain;
                }
                Ok(self.save_group(g)?)
            }
            ChatEvent::GrpLeave => {
                let mut m = self.member(group, relay)?;
                m.status = MemberStatus::Left;
                self.save_member(m)?;
                self.set_relay_status(group, relay, crate::engine::records::RelayStatus::Inactive)
            }
            ChatEvent::FileChunk { .. }
            | ChatEvent::InfoProbe(_)
            | ChatEvent::InfoProbeCheck(_)
            | ChatEvent::Ok => Ok(()),
            ChatEvent::MsgNew(_)
            | ChatEvent::MsgFileDescr { .. }
            | ChatEvent::MsgUpdate { .. }
            | ChatEvent::MsgDel { .. }
            | ChatEvent::MsgDeleted
            | ChatEvent::MsgReact { .. }
            | ChatEvent::File(_)
            | ChatEvent::FileAcpt { .. }
            | ChatEvent::FileAcptInv { .. }
            | ChatEvent::FileCancel { .. }
            | ChatEvent::Info { .. }
            | ChatEvent::Contact { .. }
            | ChatEvent::Member { .. }
            | ChatEvent::DirectDel
            | ChatEvent::GrpInv(_)
            | ChatEvent::GrpAcpt { .. }
            | ChatEvent::GrpLinkInv(_)
            | ChatEvent::GrpLinkReject(_)
            | ChatEvent::GrpLinkAcpt { .. }
            | ChatEvent::GrpRelayInv(_)
            | ChatEvent::GrpRelayAcpt { .. }
            | ChatEvent::GrpRelayTest { .. }
            | ChatEvent::GrpRelayNew { .. }
            | ChatEvent::GrpRelayReject { .. }
            | ChatEvent::GrpMemInv { .. }
            | ChatEvent::GrpMemFwd { .. }
            | ChatEvent::GrpMemInfo { .. }
            | ChatEvent::GrpMemRole { .. }
            | ChatEvent::GrpMemRestrict { .. }
            | ChatEvent::GrpMemCon { .. }
            | ChatEvent::GrpMemConAll { .. }
            | ChatEvent::GrpMemDel { .. }
            | ChatEvent::GrpDel
            | ChatEvent::GrpInfo(_)
            | ChatEvent::GrpPrefs(_)
            | ChatEvent::GrpDirectInv { .. }
            | ChatEvent::GrpRoster { .. }
            | ChatEvent::GrpRosterRequest { .. }
            | ChatEvent::GrpMsgForward { .. }
            | ChatEvent::InfoProbeOk(_)
            | ChatEvent::CallInv { .. }
            | ChatEvent::CallOffer { .. }
            | ChatEvent::CallAnswer { .. }
            | ChatEvent::CallExtra { .. }
            | ChatEvent::CallEnd { .. }
            | ChatEvent::Unknown { .. } => Err(dropped("a relay's message not taken in a channel")),
        }
    }

    /// An owner a relay introduces, or a subscriber it announces to an owner: kept with its
    /// profile and key, never connected to.
    fn on_relay_intro(&mut self, group: i64, info: MemberInfo) -> Result<()> {
        if info.member_id == self.group(group)?.my_member_id {
            return Ok(());
        }
        if let Some(mut m) = self.member_by_id(group, &info.member_id).cloned() {
            m.profile = info.profile;
            if m.key.is_none() {
                m.key = info.member_key;
            }
            m.chat_v = info.v.or(m.chat_v);
            return Ok(self.save_member(m)?);
        }
        let status = MemberStatus::Announced;
        self.new_member(group, &info, MemberCategory::Post, status, None)?;
        Ok(())
    }
}
