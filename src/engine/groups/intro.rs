//! The host introduces a new member: `x.grp.mem.new` to all, kept for those still connecting, the `x.grp.mem.intro`s batched to it, and its `x.grp.mem.inv` passed on as `x.grp.mem.fwd` (Internal.hs `introduceToAll`, `introduceMember`; Subscriber.hs `xGrpMemInv`).

use super::batch_out::batch_bodies;
use super::conn_info::member_info;
use super::role::at_least;
use crate::chat::group::{GroupMemberRole, IntroInvitation};
use crate::chat::ids::MemberId;
use crate::chat::version::GROUP_BINARY_BATCH;
use crate::chat::{ChatEvent, ChatMessage};
use crate::engine::agent_port::{AgentPort, ConnId};
use crate::engine::error::{Code, Result, fail};
use crate::engine::machine::Engine;
use crate::engine::records::{GroupRecord, MemberCategory, MemberRecord, MemberRelation};

fn shuffle<T>(v: &mut [T]) -> Result<()> {
    for i in (1..v.len()).rev() {
        let mut b = [0u8; 4];
        getrandom::fill(&mut b).map_err(|_| fail(Code::RequestFailed, "no randomness"))?;
        let j = usize::try_from(u32::from_le_bytes(b)).unwrap_or(0) % (i + 1);
        v.swap(i, j);
    }
    Ok(())
}

/// `shuffleMembers`: admins with pictures, admins, others with pictures, others; each shuffled.
fn intro_order(members: Vec<MemberRecord>) -> Result<Vec<MemberRecord>> {
    let mut parts: [Vec<MemberRecord>; 4] = Default::default();
    for m in members {
        let admin = at_least(&m.role, &GroupMemberRole::Admin);
        parts[usize::from(!admin) * 2 + usize::from(m.profile.image.is_none())].push(m);
    }
    for p in &mut parts {
        shuffle(p)?;
    }
    Ok(parts.into_iter().flatten().collect())
}

impl<A: AgentPort> Engine<A> {
    /// Called once the host's connection with `member` works. `shouldIntroduce`: every
    /// current member it has no relation with yet; one still connecting gets its
    /// `x.grp.mem.new` once it connects, or from its inviter.
    pub(crate) fn introduce_to_all(&mut self, group: i64, member: i64) -> Result<()> {
        let m = self.member(group, member)?;
        let conn = m
            .reachable()
            .ok_or_else(|| fail(Code::RequestFailed, "member connection not active"))?;
        let recipients: Vec<MemberRecord> = self
            .state
            .members_of(group)
            .filter(|r| r.id != member && r.status.current())
            .filter(|r| !m.relations.contains_key(&r.id))
            .cloned()
            .collect();
        let g = self.group(group)?;
        let new = ChatMessage::new(ChatEvent::GrpMemNew {
            member_info: member_info(&g, &m),
            scope: None,
        })?;
        let ids: Vec<i64> = recipients.iter().map(|r| r.id).collect();
        self.send_to_members(group, &ids, &new)?;
        self.send_intros(&g, &m, conn, intro_order(recipients)?)
    }

    fn send_intros(
        &mut self,
        g: &GroupRecord,
        m: &MemberRecord,
        conn: ConnId,
        told: Vec<MemberRecord>,
    ) -> Result<()> {
        let mut elements = Vec::with_capacity(told.len());
        for r in &told {
            self.relate(m.group, m.id, r.id, MemberRelation::Introduced)?;
            let intro = ChatMessage::new(ChatEvent::GrpMemIntro {
                member_info: member_info(g, r),
                restrictions: None,
            })?;
            elements.push(intro.to_json()?);
        }
        let binary = m.chat_v.is_some_and(|v| v.supports(GROUP_BINARY_BATCH));
        let (bodies, dropped) = batch_bodies(elements, binary)?;
        if dropped > 0 {
            self.log
                .line(format_args!("{dropped} introductions too large to send"));
        }
        for body in bodies {
            self.agent
                .send(&mut self.store, &conn, self.state.pq_of(&conn), body)?;
        }
        Ok(())
    }

    /// `xGrpMemInv`: only from a member the user invited, about a member it was introduced to;
    /// one still connecting gets it once it connects (`sendGroupMemberMessage`).
    pub(crate) fn on_mem_inv(
        &mut self,
        group: i64,
        sender: i64,
        member_id: &MemberId,
        intro: IntroInvitation,
    ) -> Result<()> {
        let from = self.member(group, sender)?;
        if from.category != MemberCategory::Invitee {
            self.log.line(format_args!(
                "x.grp.mem.inv can be only sent by invitee member"
            ));
            return Ok(());
        }
        let Some(to) = self.member_by_id(group, member_id).map(|m| m.id) else {
            self.log.line(format_args!(
                "x.grp.mem.inv error: referenced member does not exist"
            ));
            return Ok(());
        };
        let fwd = ChatMessage::new(ChatEvent::GrpMemFwd {
            member_info: member_info(&self.group(group)?, &from),
            intro,
        })?;
        self.send_to_members(group, &[to], &fwd)?;
        Ok(())
    }
}
