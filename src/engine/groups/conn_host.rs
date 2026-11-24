//! The user as host, on the connection with a member it invited: CONF with `x.grp.acpt` answered with `x.grp.mem.info`, then CON, the introductions and the history (Subscriber.hs:797-808, :947-985).

use crate::types::model::ChatNotice;

use super::conn::apply_handshake;
use super::role::history_on;
use crate::chat::{ChatEvent, ChatMessage};
use crate::engine::agent_port::{AgentPort, ConnId};
use crate::engine::error::Result;
use crate::engine::machine::Engine;
use crate::engine::records::{MemberRecord, MemberStatus};

const fn pending(s: MemberStatus) -> bool {
    matches!(
        s,
        MemberStatus::PendingApproval | MemberStatus::PendingReview
    )
}

impl<A: AgentPort> Engine<A> {
    pub(crate) fn on_invitee_conf(
        &mut self,
        group: i64,
        member: i64,
        conn: &ConnId,
        msg: ChatMessage,
    ) -> Result<()> {
        let ChatEvent::GrpAcpt {
            member_id,
            member_key,
            profile,
        } = &msg.event
        else {
            self.log.line(format_args!(
                "CONF from invited member must have x.grp.acpt"
            ));
            return Ok(());
        };
        let mut m = self.member(group, member)?;
        if *member_id != m.member_id {
            self.log.line(format_args!(
                "x.grp.acpt: memberId is different from expected"
            ));
            return Ok(());
        }
        if m.status != MemberStatus::Invited {
            self.log.line(format_args!("x.grp.acpt again: dropped"));
            return Ok(());
        }
        let g = self.group(group)?;
        // 7.1.4 answers with the host's `x.grp.mem.info`, not `x.ok` (Subscriber.hs:807).
        let info = self.my_mem_info(&g)?;
        self.agent.allow(&mut self.store, conn, info)?;
        m.status = MemberStatus::Accepted;
        if member_key.is_some() {
            m.key.clone_from(member_key);
        }
        apply_handshake(&g, &mut m, &msg, profile.as_ref());
        self.save_member(m)?;
        self.state.chats_dirty = true;
        Ok(())
    }

    /// CON with an invitee: connected, told, and introduced to everyone (`introduceToAll`); one
    /// who came by the group's link is the link's to admit first.
    /// `flushed`: the user's items that just reached it from the queue, left out of history.
    pub(crate) fn on_invitee_con(
        &mut self,
        group: i64,
        mut m: MemberRecord,
        flushed: &[i64],
    ) -> Result<()> {
        let waits = pending(m.status);
        if !waits {
            m.status = MemberStatus::Connected;
        }
        let (id, via_link, name) = (m.id, m.via_link, m.profile.display_name.clone());
        self.save_member(m)?;
        self.group_notice(group, Some(id), ChatNotice::MemberJoined(name))?;
        let introduce = if via_link {
            self.on_link_member_con(group, id)?
        } else {
            !waits
        };
        if !introduce {
            return Ok(());
        }
        self.introduce_to_all(group, id)?;
        if history_on(&self.group(group)?) {
            self.send_history(group, id, flushed)?;
        }
        Ok(())
    }
}
