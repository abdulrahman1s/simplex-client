//! BlockMemberForAll: a moderator hides a member's messages for everyone (Commands.hs `APIBlockMembersForAll`).

use crate::types::model::ChatNotice;

use crate::chat::group::{GroupMemberRole, MemberRestrictionStatus, MemberRestrictions};
use crate::chat::{ChatEvent, ChatMessage};
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::groups::rank::{assert_user_role, rank};
use crate::engine::machine::Engine;
use crate::engine::records::MemberStatus;

impl<A: AgentPort> Engine<A> {
    /// The user needs moderator and at least the member's role; the other current members are
    /// told, not the member.
    pub(crate) fn block_member_for_all(&mut self, group: i64, member: i64, on: bool) -> Result<()> {
        let g = self.group(group)?;
        let mut m = self.member(group, member)?;
        if matches!(
            m.status,
            MemberStatus::PendingApproval | MemberStatus::PendingReview
        ) {
            return Err(fail(
                Code::RequestFailed,
                "can't block/unblock members pending approval",
            ));
        }
        let need = [GroupMemberRole::Moderator, m.role.clone()]
            .into_iter()
            .max_by_key(rank)
            .unwrap_or(GroupMemberRole::Moderator);
        assert_user_role(&g, &need)?;
        if m.blocked_by_admin == on {
            return Ok(());
        }
        let restriction = if on {
            MemberRestrictionStatus::Blocked
        } else {
            MemberRestrictionStatus::Unrestricted
        };
        let msg = ChatMessage::new(ChatEvent::GrpMemRestrict {
            member_id: m.member_id.clone(),
            restrictions: MemberRestrictions { restriction },
        })?;
        self.send_to_group(group, &msg, Some(member))?;
        m.blocked_by_admin = on;
        let who = m.profile.display_name.clone();
        self.save_member(m)?;
        let notice = ChatNotice::MemberBlocked {
            by: None,
            who,
            blocked: on,
        };
        self.group_notice(group, None, notice)?;
        self.members(group)
    }
}
