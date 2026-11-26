//! An owner's relays invited: each configured address read, then joined with a signed `x.grp.relay.inv` (Commands.hs `addRelays`; Internal.hs `encodeConnInfoSigning`).

use crate::agent::connection::LinkLookup;
use crate::agent::links::ConnectionLink;
use crate::chat::ChatEvent;
use crate::chat::channel::GroupRelayInvitation;
use crate::chat::group::{GroupMemberRole, MemberIdRole};
use crate::chat::ids::b64_encode;
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::machine::Engine;
use crate::engine::records::{
    ConnKind, ConnRecord, Lookup, MemberCategory, RelayRecord, RelayStatus,
};
use crate::types::model::ChatRelay;

use super::keys::public_b64;

fn dropped(why: &str) -> crate::engine::error::EngineError {
    fail(Code::RequestFailed, why.to_owned())
}

impl<A: AgentPort> Engine<A> {
    /// Every configured relay, once the channel's link exists.
    pub(crate) fn invite_channel_relays(&mut self, group: i64) -> Result<()> {
        let relays = self.state.settings.chat_relays.clone();
        self.invite_relays(group, &relays)
    }

    /// `addRelays`: each relay a member, then its address read; what follows is the same for a
    /// new channel and for relays added later.
    pub(super) fn invite_relays(&mut self, group: i64, relays: &[ChatRelay]) -> Result<()> {
        for r in relays {
            let member = self.new_relay_member(group, MemberCategory::Invitee, None)?;
            let mut m = self.member(group, member)?;
            m.profile.display_name.clone_from(&r.name);
            self.save_member(m)?;
            let mut g = self.group(group)?;
            if let Some(c) = g.channel.as_mut() {
                c.relays.push(RelayRecord {
                    member,
                    address: r.address.clone(),
                    name: r.name.clone(),
                    status: RelayStatus::New,
                    link: None,
                    web_domain: None,
                });
            }
            self.save_group(g)?;
            self.relay_changed(group, member)?;
            if let Err(e) = self.look_up(&r.address, Lookup::RelayAddress { group, member }) {
                self.log
                    .line(format_args!("a chat relay's address failed: {}", e.detail));
                self.relay_failed(group, member)?;
            }
        }
        Ok(())
    }

    /// The relay's address read: join it, the invitation signed with the owner's member key
    /// over the channel's id and the owner's member id (`groupMsgSigning`, `requiresSignature`).
    pub(super) fn on_relay_address(
        &mut self,
        group: i64,
        member: i64,
        data: LinkLookup,
    ) -> Result<()> {
        let g = self.group(group)?;
        let c = g.channel.clone().ok_or_else(|| dropped("not a channel"))?;
        let mut m = self.member(group, member)?;
        let inv = GroupRelayInvitation {
            from_member: MemberIdRole {
                member_id: g.my_member_id.clone(),
                member_role: GroupMemberRole::Owner,
            },
            from_member_profile: self.handshake_profile(&g),
            relay_member_id: m.member_id.clone(),
            group_link: c.link.clone(),
            public_group_id: Some(b64_encode(&c.public_group_id)),
            from_member_key: Some(public_b64(&c.member_private)?),
        };
        let info = self.signed_conn_info(&g, ChatEvent::GrpRelayInv(inv))?;
        // Owner to relay is PQ off (`prepareConnectionToJoin … PQSupportOff`).
        let conn = self.agent.join(
            &mut self.store,
            &ConnectionLink::Full(data.conn_req),
            info,
            false,
        )?;
        m.conn = Some(conn);
        self.save_member(m)?;
        self.save_conn(ConnRecord {
            conn,
            kind: ConnKind::Member { group, member },
        })?;
        self.set_relay_status(group, member, RelayStatus::Invited)
    }
}
