//! RemoveMember: an admin removes a member, maybe with what they wrote (Commands.hs `APIRemoveMembers`).

use crate::types::model::ChatNotice;

use crate::chat::group::GroupMemberRole;
use crate::chat::{ChatEvent, ChatMessage};
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::groups::rank::{assert_user_role, rank};
use crate::engine::machine::Engine;
use crate::engine::records::MemberStatus;

impl<A: AgentPort> Engine<A> {
    /// The user needs admin and at least the member's role. A current member's removal goes to
    /// every current member, the member too, before their connection goes once it is
    /// delivered; an invited one's connection just goes.
    pub(crate) fn remove_member(&mut self, group: i64, member: i64, messages: bool) -> Result<()> {
        let g = self.group(group)?;
        let m = self.member(group, member)?;
        if m.status.gone() {
            return Ok(());
        }
        let need = [GroupMemberRole::Admin, m.role.clone()]
            .into_iter()
            .max_by_key(rank)
            .unwrap_or(GroupMemberRole::Admin);
        assert_user_role(&g, &need)?;
        match m.status {
            MemberStatus::PendingApproval | MemberStatus::PendingReview => {
                return Err(fail(
                    Code::RequestFailed,
                    "a member pending approval is answered in their support chat",
                ));
            }
            MemberStatus::Invited => {}
            _ => {
                let msg = ChatMessage::new(ChatEvent::GrpMemDel {
                    member_id: m.member_id.clone(),
                    messages,
                    roster_version: None,
                })?;
                self.send_to_group(group, &msg, None)?;
                if messages {
                    self.mark_member_items_deleted(group, Some(member))?;
                }
            }
        }
        let invited = m.status == MemberStatus::Invited;
        self.drop_member_conn(group, member)?;
        let mut m = self.member(group, member)?;
        let who = m.profile.display_name.clone();
        m.status = MemberStatus::Removed;
        self.save_member(m)?;
        self.state.chats_dirty = true;
        if invited {
            return self.members(group);
        }
        let notice = ChatNotice::MemberRemoved { by: None, who };
        self.group_notice(group, None, notice)?;
        self.members(group)
    }
}
