//! A moderator blocks or unblocks a member for everyone (Subscriber.hs `xGrpMemRestrict`; Store/Groups.hs `updateGroupMemberBlocked`).

use crate::types::model::ChatNotice;

use super::admin::insufficient;
use super::inbound::GroupCtx;
use super::rank::at_least;
use crate::chat::group::{GroupMemberRole, MemberRestrictionStatus, MemberRestrictions};
use crate::chat::ids::MemberId;
use crate::engine::agent_port::AgentPort;
use crate::engine::error::Result;
use crate::engine::machine::Engine;

impl<A: AgentPort> Engine<A> {
    /// Subscriber.hs 3716. The user is never restricted by it: it is sent to the restricted
    /// member too, for efficiency. A member the user does not know yet is added, blocked.
    pub(crate) fn on_mem_restrict(
        &mut self,
        ctx: GroupCtx,
        member_id: &MemberId,
        restrictions: &MemberRestrictions,
    ) -> Result<()> {
        if *member_id == self.group(ctx.group)?.my_member_id {
            return Ok(());
        }
        let sender = self.member(ctx.group, ctx.author)?;
        if !at_least(&sender.role, &GroupMemberRole::Moderator) {
            return Err(insufficient("x.grp.mem.restrict"));
        }
        let Some(mut target) = self.author_of_forward(ctx.group, member_id, "", true)? else {
            return Ok(());
        };
        let blocked = restrictions.restriction == MemberRestrictionStatus::Blocked;
        if target.blocked_by_admin == blocked {
            return Ok(());
        }
        if !at_least(&sender.role, &target.role) {
            return Err(insufficient("x.grp.mem.restrict"));
        }
        target.blocked_by_admin = blocked;
        let who = target.profile.display_name.clone();
        self.save_member(target)?;
        let by = Some(sender.profile.display_name);
        let notice = ChatNotice::MemberBlocked { by, who, blocked };
        self.group_notice(ctx.group, Some(ctx.author), notice)?;
        if let Err(e) = self.members(ctx.group) {
            self.log
                .line(format_args!("members not listed: {}", e.detail));
        }
        Ok(())
    }
}
