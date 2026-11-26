//! How a channel shows: its link, id and relays on the group (`GroupInfo.useRelays`, `APIGetGroupRelays`), and a relay's change told as it happens (`CEvtGroupRelayUpdated`).

use crate::chat::group::GroupMemberRole;
use crate::chat::ids::b64_encode;
use crate::engine::agent_port::AgentPort;
use crate::engine::error::Result;
use crate::engine::machine::Engine;
use crate::engine::records::{ChannelRecord, GroupRecord, MemberRecord, MemberStatus, RelayStatus};
use crate::types::Event;
use crate::types::model::{ChannelInfo, ChannelRelay};

fn status_wire(s: RelayStatus) -> crate::types::model::RelayStatus {
    use crate::types::model::RelayStatus as W;
    match s {
        RelayStatus::New => W::New,
        RelayStatus::Invited => W::Invited,
        RelayStatus::Accepted => W::Accepted,
        RelayStatus::AcknowledgedRoster => W::AcknowledgedRoster,
        RelayStatus::Active => W::Active,
        RelayStatus::Inactive => W::Inactive,
        RelayStatus::Rejected => W::Rejected,
    }
}

impl<A: AgentPort> Engine<A> {
    fn relay_wire(&self, g: &GroupRecord, m: &MemberRecord) -> ChannelRelay {
        let own = g.channel.as_ref().and_then(|c| c.relay(m.id));
        ChannelRelay {
            member: m.id,
            name: own.map_or_else(|| m.profile.display_name.clone(), |r| r.name.clone()),
            status: own.map(|r| status_wire(r.status)),
            link: own
                .and_then(|r| r.link.clone())
                .or_else(|| m.relay_link.clone()),
            connected: matches!(m.status, MemberStatus::Connected | MemberStatus::Complete),
            web_domain: own.and_then(|r| r.web_domain.clone()),
            pq: m.conn.is_some_and(|c| self.agent.pq_on(&c)),
        }
    }

    /// The channel part of a group's wire form.
    pub(crate) fn channel_wire(&self, g: &GroupRecord) -> Option<ChannelInfo> {
        let c = g.channel.as_ref()?;
        let relays = (self.state.members_of(g.id))
            .filter(|m| m.role == GroupMemberRole::Relay)
            .map(|m| self.relay_wire(g, m))
            .collect();
        Some(ChannelInfo {
            use_relays: true,
            link: c.link.clone(),
            public_group_id: b64_encode(&c.public_group_id),
            owner: c.owner(),
            relays,
        })
    }

    /// `CEvtGroupRelayUpdated`: nobody asked, so it is an event of its own.
    pub(crate) fn relay_changed(&mut self, group: i64, member: i64) -> Result<()> {
        let g = self.group(group)?;
        let m = self.member(group, member)?;
        let relay = self.relay_wire(&g, &m);
        self.emit(Event::ChannelRelay { group, relay });
        self.state.chats_dirty = true;
        Ok(())
    }

    /// A relay that can't be reached or refused: an owner's turns `Rejected`; a subscriber's
    /// stays as it was, as one relay is enough.
    pub(crate) fn relay_failed(&mut self, group: i64, member: i64) -> Result<()> {
        let owner = self
            .state
            .groups
            .get(&group)
            .and_then(|g| g.channel.as_ref())
            .is_some_and(ChannelRecord::owner);
        if owner {
            self.set_relay_status(group, member, RelayStatus::Rejected)?;
        }
        Ok(())
    }

    pub(super) fn set_relay_status(
        &mut self,
        group: i64,
        member: i64,
        status: RelayStatus,
    ) -> Result<()> {
        let mut g = self.group(group)?;
        let Some(r) = g.channel.as_mut().and_then(|c| c.relay_mut(member)) else {
            return Ok(());
        };
        if r.status == status {
            return Ok(());
        }
        r.status = status;
        self.save_group(g)?;
        self.relay_changed(group, member)
    }
}
