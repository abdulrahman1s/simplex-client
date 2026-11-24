//! Accepting a group member's invitation: its link joined with our profile, PQ off, and the member's friend made or moved onto the new connection (Commands.hs `APIAcceptMemberContact`; Store/Groups.hs `createMemberContactInvited`, `updateMemberContactInvited`).

use crate::types::Event;
use crate::types::model::{ChatRef, MessageAuthor, MessageBody};

use crate::agent::links::ConnectionLink;
use crate::engine::agent_port::{AgentPort, ConnId};
use crate::engine::error::{Code, Result, fail};
use crate::engine::machine::Engine;
use crate::engine::profile::conn_info;
use crate::engine::records::{
    ConnKind, ConnRecord, FriendRecord, ItemRecord, MemberInvite, RequestRecord, keys,
};

/// `joinNewConn`: 7.1.4 joins a member's link with PQ off.
const PQ: bool = false;

impl<A: AgentPort> Engine<A> {
    pub(crate) fn accept_member_invite(
        &mut self,
        req: RequestRecord,
        inv: MemberInvite,
    ) -> Result<()> {
        let link =
            ConnectionLink::parse(&inv.link).map_err(|e| fail(Code::BadLink, e.to_string()))?;
        let ours = conn_info(&self.state.user)?;
        let conn = self.agent.join(&mut self.store, &link, ours, PQ)?;
        let member = self.state.members.get(&(inv.group, inv.member)).cloned();
        let known = member.as_ref().and_then(|m| m.friend);
        let id = match known.filter(|f| self.state.friends.contains_key(f)) {
            Some(f) => self.move_friend(f, conn)?,
            None => self.new_member_friend(&req, conn)?,
        };
        self.save_conn(ConnRecord {
            conn,
            kind: ConnKind::Friend { id },
        })?;
        if let Some(mut m) = member {
            m.friend = Some(id);
            self.save_member(m)?;
        }
        if let Some(text) = inv.text {
            let item = ItemRecord::new(MessageAuthor::Friend, MessageBody::Text(text), req.since);
            let m = self.add_item(ChatRef::Direct(id), &item)?;
            self.emit(Event::Messages(vec![m]));
        }
        self.forget_request(req.id, false)?;
        self.state.chats_dirty = true;
        Ok(())
    }

    fn new_member_friend(&mut self, req: &RequestRecord, conn: ConnId) -> Result<i64> {
        let id = self.store.next_id(keys::FRIEND_SEQ)?;
        let mut f = FriendRecord::new(id, conn, self.clock.wall_ms());
        f.name.clone_from(&req.name);
        f.about.clone_from(&req.about);
        f.image.clone_from(&req.image);
        f.pq = PQ;
        self.save_friend(f)?;
        Ok(id)
    }

    /// `updateMemberContactInvited`: the friend is active again on the new connection, the old
    /// one goes, and so does a verification of it (`securityCodeChanged`).
    fn move_friend(&mut self, id: i64, conn: ConnId) -> Result<i64> {
        let mut f = self.friend(id)?;
        let old = std::mem::replace(&mut f.conn, conn);
        f.ready = false;
        f.gone = false;
        f.pq = PQ;
        self.save_friend(f)?;
        self.forget_conn(&old)?;
        self.security_code_changed(id)?;
        Ok(id)
    }
}
