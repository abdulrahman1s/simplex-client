//! An owner's relay answers: `x.grp.relay.acpt` names its link for the channel, which must name the relay before the connection is allowed with `x.ok`; `x.grp.relay.reject` leaves it rejected (Subscriber.hs `XGrpRelayAcpt`, `XGrpRelayReject`, `CFGetRelayDataAccept`).

use crate::agent::connection::LinkLookup;
use crate::agent::short_link::{UserLinkData, decode_short_link_data};
use crate::chat::channel::RelayShortLinkData;
use crate::chat::ids::b64_encode;
use crate::chat::{ChatEvent, ChatMessage};
use crate::engine::agent_port::{AgentPort, ConnId};
use crate::engine::error::{Code, Result, fail};
use crate::engine::groups::group_conn_info;
use crate::engine::machine::Engine;
use crate::engine::records::{Lookup, MemberStatus, RelayStatus};

fn dropped(why: &str) -> crate::engine::error::EngineError {
    fail(Code::RequestFailed, why.to_owned())
}

impl<A: AgentPort> Engine<A> {
    /// The relay's answer in its CONF.
    pub(super) fn on_owner_relay_conf(
        &mut self,
        group: i64,
        member: i64,
        conn: &ConnId,
        msg: ChatMessage,
    ) -> Result<()> {
        match msg.event {
            ChatEvent::GrpRelayAcpt {
                relay_link,
                relay_cap,
            } => {
                let mut m = self.member(group, member)?;
                m.relay_link = Some(relay_link.clone());
                self.save_member(m)?;
                let mut g = self.group(group)?;
                if let Some(r) = g.channel.as_mut().and_then(|c| c.relay_mut(member)) {
                    r.link = Some(relay_link.clone());
                    r.web_domain = relay_cap.web_domain;
                }
                self.save_group(g)?;
                self.look_up(&relay_link, Lookup::RelayAccept { group, member })
            }
            // `GSMemLeft`, so the relay reads as one that left.
            ChatEvent::GrpRelayReject { reason } => {
                self.log.line(format_args!(
                    "a chat relay refused the channel: {}",
                    reason.as_str()
                ));
                let mut m = self.member(group, member)?;
                m.status = MemberStatus::Left;
                self.save_member(m)?;
                self.agent
                    .allow(&mut self.store, conn, group_conn_info(ChatEvent::Ok)?)?;
                self.set_relay_status(group, member, RelayStatus::Rejected)
            }
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
            | ChatEvent::GrpLinkMem { .. }
            | ChatEvent::GrpLinkAcpt { .. }
            | ChatEvent::GrpRelayInv(_)
            | ChatEvent::GrpRelayTest { .. }
            | ChatEvent::GrpRelayNew { .. }
            | ChatEvent::GrpRelayCap(_)
            | ChatEvent::GrpMemNew { .. }
            | ChatEvent::GrpMemIntro { .. }
            | ChatEvent::GrpMemInv { .. }
            | ChatEvent::GrpMemFwd { .. }
            | ChatEvent::GrpMemInfo { .. }
            | ChatEvent::GrpMemRole { .. }
            | ChatEvent::GrpMemRestrict { .. }
            | ChatEvent::GrpMemCon { .. }
            | ChatEvent::GrpMemConAll { .. }
            | ChatEvent::GrpMemDel { .. }
            | ChatEvent::GrpLeave
            | ChatEvent::GrpDel
            | ChatEvent::GrpInfo(_)
            | ChatEvent::GrpPrefs(_)
            | ChatEvent::GrpDirectInv { .. }
            | ChatEvent::GrpRoster { .. }
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
                Err(dropped("CONF from a relay must have x.grp.relay.acpt"))
            }
        }
    }

    /// `CFGetRelayDataAccept`: the relay link's entity id is the member id the owner gave the
    /// relay; its key and profile are the relay's. Then the connection is allowed with `x.ok`.
    pub(super) fn on_relay_link_accepted(
        &mut self,
        group: i64,
        member: i64,
        data: LinkLookup,
    ) -> Result<()> {
        let mut m = self.member(group, member)?;
        if data.entity_id.as_deref() != Some(m.member_id.as_bytes()) {
            return Err(dropped("relay link: relay member ID mismatch"));
        }
        let relay: RelayShortLinkData = decode_short_link_data(&UserLinkData(data.user_data))
            .ok()
            .flatten()
            .ok_or_else(|| dropped("relay link: no relay link data"))?;
        m.key = Some(b64_encode(&data.root_key.to_der()));
        m.profile = relay.relay_profile;
        m.status = MemberStatus::Accepted;
        let conn = m
            .conn
            .ok_or_else(|| dropped("relay without a connection"))?;
        self.save_member(m)?;
        self.agent
            .allow(&mut self.store, &conn, group_conn_info(ChatEvent::Ok)?)?;
        self.set_relay_status(group, member, RelayStatus::Accepted)
    }
}
