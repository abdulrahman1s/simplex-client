//! A channel relay's connection events, each to the owner's or the subscriber's side (Subscriber.hs `processGroupMessage` for `isRelay` members).

use crate::engine::agent_port::{AgentPort, ConnId};
use crate::engine::error::Result;
use crate::engine::groups::parse_conn_info;
use crate::engine::machine::Engine;

impl<A: AgentPort> Engine<A> {
    /// `Some(owner)` when `member` is a relay of a channel: whether the user owns it.
    pub(crate) fn relay_side(&self, group: i64, member: i64) -> Option<bool> {
        let g = self.state.groups.get(&group)?;
        let c = g.channel.as_ref()?;
        let m = self.state.members.get(&(group, member))?;
        (m.role == crate::chat::group::GroupMemberRole::Relay).then_some(c.owner())
    }

    pub(crate) fn on_relay_conf(
        &mut self,
        group: i64,
        member: i64,
        conn: &ConnId,
        info: &[u8],
        owner: bool,
    ) -> Result<()> {
        let msg = parse_conn_info(info)?;
        if owner {
            self.on_owner_relay_conf(group, member, conn, msg)
        } else {
            self.on_subscriber_conf(group, member, conn, msg)
        }
    }

    pub(crate) fn on_relay_con(&mut self, group: i64, member: i64, owner: bool) -> Result<()> {
        if owner {
            self.on_owner_relay_con(group, member)
        } else {
            self.on_subscriber_con(group, member)
        }
    }
}
