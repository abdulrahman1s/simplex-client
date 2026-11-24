//! Joins through the group's link that have not connected yet, at most `MAX_LINK_JOINS` per group.

use crate::engine::agent_port::AgentPort;
use crate::engine::error::Result;
use crate::engine::machine::Engine;
use crate::engine::records::{MemberCategory, MemberStatus, NS, keys};

/// Joins waiting for their connection; the oldest is dropped to make room for a new one.
/// 7.1.4 accepts group link requests without limit, each with a member, a connection and queues.
pub const MAX_LINK_JOINS: usize = 50;

impl<A: AgentPort> Engine<A> {
    /// Drops the oldest unconnected link joins of `group` until one more fits: its connection
    /// and its record, as nobody has met it yet.
    pub(crate) fn room_for_link_join(&mut self, group: i64) -> Result<()> {
        let mut waiting: Vec<i64> = (self.state.members_of(group))
            .filter(|m| {
                m.via_link
                    && m.category == MemberCategory::Invitee
                    && m.status == MemberStatus::Accepted
            })
            .map(|m| m.id)
            .collect();
        waiting.sort_unstable();
        let over = (waiting.len() + 1).saturating_sub(MAX_LINK_JOINS);
        for id in waiting.into_iter().take(over) {
            self.drop_member_conn(group, id)?;
            self.store.delete(NS, &keys::member(group, id))?;
            self.state.members.remove(&(group, id));
            self.log
                .line(format_args!("an unconnected group link join was dropped"));
        }
        Ok(())
    }
}
