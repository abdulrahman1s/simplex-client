//! SetMemberRole: an admin changes a member's role and tells the group (Commands.hs `APIMembersRole`).

use crate::types::model::{ChatNotice, MemberRole};

use super::convert::{to_chat, to_wire};
use crate::chat::group::GroupMemberRole;
use crate::chat::{ChatEvent, ChatMessage};
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::groups::rank::{assert_user_role, rank};
use crate::engine::machine::Engine;
use crate::engine::records::MemberStatus;

impl<A: AgentPort> Engine<A> {
    /// The user needs admin, and at least the member's role and the new one. A current member's
    /// change goes to every current member, the member too; an invited one is sent the
    /// invitation again.
    pub(crate) fn set_member_role(
        &mut self,
        group: i64,
        member: i64,
        role: MemberRole,
    ) -> Result<()> {
        let g = self.group(group)?;
        let mut m = self.member(group, member)?;
        let new = to_chat(role);
        if m.role == new {
            return Ok(());
        }
        if matches!(
            m.status,
            MemberStatus::PendingApproval | MemberStatus::PendingReview
        ) {
            return Err(fail(
                Code::RequestFailed,
                "can't change role of members pending approval",
            ));
        }
        if m.role == GroupMemberRole::Relay {
            return Err(fail(Code::RequestFailed, "relay role can't be changed"));
        }
        let need = [GroupMemberRole::Admin, m.role.clone(), new.clone()]
            .into_iter()
            .max_by_key(rank)
            .unwrap_or(GroupMemberRole::Admin);
        assert_user_role(&g, &need)?;
        if m.status == MemberStatus::Invited {
            let link = m.invitation.clone();
            m.role = new;
            self.save_member(m)?;
            return match link {
                Some(link) => self.send_group_invitation(group, member, &link),
                None => Ok(()),
            };
        }
        if !m.status.current() {
            return Err(fail(Code::RequestFailed, "not a member of the group"));
        }
        let msg = ChatMessage::new(ChatEvent::GrpMemRole {
            member_id: m.member_id.clone(),
            role: new.clone(),
            member_key: None,
            roster_version: None,
        })?;
        self.send_to_group(group, &msg, None)?;
        let who = m.profile.display_name.clone();
        m.role = new;
        self.save_member(m)?;
        let notice = ChatNotice::MemberRole {
            by: None,
            who,
            role: to_wire(&to_chat(role)),
        };
        self.group_notice(group, None, notice)?;
        self.state.chats_dirty = true;
        Ok(())
    }
}
