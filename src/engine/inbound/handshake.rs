//! The invitation handshake (Agent.hs `CONF`/`INFO`/`CON`): our link out, their profile in, ready.

use crate::types::model::{ChatNotice, ChatRef, MessageAuthor, MessageBody};
use crate::types::{Event, RequestKind};

use crate::engine::agent_port::{AgentFailure, AgentPort, ConnId};
use crate::engine::error::{Code, EngineError, Result, fail};
use crate::engine::machine::Engine;
use crate::engine::profile::{conn_info, received_profile, user_for};
use crate::engine::records::{ConnKind, ConnRecord, FriendRecord, ItemRecord, keys};
use crate::engine::relays::Retry;
use crate::engine::timed::adopt::apply_profile;

fn unknown() -> EngineError {
    fail(Code::RequestFailed, "an event for an unknown connection")
}

impl<A: AgentPort> Engine<A> {
    pub(crate) fn on_invitation(
        &mut self,
        conn: &ConnId,
        full: String,
        short: Option<String>,
    ) -> Result<()> {
        if let Some((group, member)) = self.state.member_of(conn) {
            return self.on_member_invitation(group, member, conn, full, short);
        }
        if let Some(friend) = self.state.friend_of(conn)
            && self.send_member_contact(friend, full.clone())?
        {
            return Ok(());
        }
        let mut rec = self.state.conns.get(conn).cloned().ok_or_else(unknown)?;
        let ConnKind::Invitation {
            created, incognito, ..
        } = rec.kind.clone()
        else {
            return Err(unknown());
        };
        let shared = short.clone().unwrap_or_else(|| full.clone());
        rec.kind = ConnKind::Invitation {
            full: Some(full),
            short,
            created,
            incognito,
        };
        self.save_conn(rec)?;
        if self.state.inviting.as_ref() == Some(conn) {
            self.state.inviting = None;
        }
        // The short link when the relay keeps one: a QR code that scans at the page's size.
        self.emit(Event::Invite(shared));
        Ok(())
    }

    /// Someone joined our invitation: they become a friend, and we answer with our profile.
    pub(crate) fn on_conf(&mut self, conn: &ConnId, info: &[u8]) -> Result<()> {
        if let Some((group, member)) = self.state.member_of(conn) {
            return self.on_member_conf(group, member, conn, info);
        }
        if let Some(r) = self.on_link_join_conf(conn, info) {
            return r;
        }
        // A connection we made for a request to an address: the owner accepted it.
        if let Some(friend) = self.state.friend_of(conn) {
            return self.on_owner_conf(friend, conn, info);
        }
        let rec = self.state.conns.get(conn).ok_or_else(unknown)?;
        let ConnKind::Invitation {
            full,
            short,
            incognito,
            ..
        } = &rec.kind
        else {
            return Err(unknown());
        };
        let incognito = incognito.clone();
        let links: Vec<String> = [full, short].into_iter().flatten().cloned().collect();
        let profile = received_profile(info)
            .ok_or_else(|| fail(Code::RequestFailed, "conn-info without a profile"))?;
        let id = self.store.next_id(keys::FRIEND_SEQ)?;
        let mut f = FriendRecord::new(id, *conn, self.clock.wall_ms());
        apply_profile(self.state.user.prefs.as_ref(), &mut f, &profile);
        f.invited_with = links;
        f.incognito.clone_from(&incognito);
        let ours = conn_info(&user_for(&self.state.user, &f))?;
        self.agent.allow(&mut self.store, conn, ours)?;
        self.save_friend(f)?;
        self.save_conn(ConnRecord {
            conn: *conn,
            kind: ConnKind::Friend { id },
        })?;
        self.state.chats_dirty = true;
        Ok(())
    }

    /// The inviting side's profile, on a link we joined.
    pub(crate) fn on_info(&mut self, conn: &ConnId, info: &[u8]) -> Result<()> {
        if let Some((group, member)) = self.state.member_of(conn) {
            return self.on_member_info(group, member, conn, info);
        }
        let id = self.state.friend_of(conn).ok_or_else(unknown)?;
        let profile = received_profile(info)
            .ok_or_else(|| fail(Code::RequestFailed, "conn-info without a profile"))?;
        let mut f = self.friend(id)?;
        apply_profile(self.state.user.prefs.as_ref(), &mut f, &profile);
        self.save_friend(f)?;
        self.refresh_friend_range_from(id, info)?;
        self.state.chats_dirty = true;
        Ok(())
    }

    pub(crate) fn on_con(&mut self, conn: &ConnId) -> Result<()> {
        if let Some((group, member)) = self.state.member_of(conn) {
            return self.on_member_con(group, member, conn);
        }
        let id = self.state.friend_of(conn).ok_or_else(unknown)?;
        let mut f = self.friend(id)?;
        if f.ready {
            return Ok(());
        }
        f.ready = true;
        self.save_friend(f)?;
        let notice = MessageBody::Notice(ChatNotice::Connected);
        let item = ItemRecord::new(MessageAuthor::Friend, notice, self.clock.wall_ms());
        let m = self.add_item(ChatRef::Direct(id), &item)?;
        self.emit(Event::Messages(vec![m]));
        self.state.chats_dirty = true;
        self.send_auto_reply(id)
    }

    /// The agent retries what it can, so an error ends nothing here: an invitation being made
    /// or a link being joined is reported as failed, and the user may ask again or remove it.
    pub(crate) fn on_conn_error(&mut self, conn: &ConnId, err: &AgentFailure) -> Result<()> {
        if let Some(r) = self.on_address_error(conn, err) {
            return r;
        }
        if let Some((group, member)) = self.state.member_of(conn) {
            return self.on_member_error(group, member, conn, err);
        }
        if let Some(ConnKind::GroupLink { group }) = self.state.conns.get(conn).map(|c| &c.kind) {
            return self.on_group_link_error(*group, err);
        }
        let e = EngineError::from(err.clone());
        if self.state.inviting.as_ref() == Some(conn) {
            if self.retry_after_check(Retry::Invite, conn, err) {
                return Ok(());
            }
            self.state.inviting = None;
            return self.fail_event(RequestKind::CreateInvite, e);
        }
        let joining = self
            .state
            .friend_of(conn)
            .and_then(|id| self.state.friends.get(&id))
            .is_some_and(|f| !f.ready && f.link.is_some());
        if !joining {
            self.log
                .line(format_args!("a connection failed: {}", err.detail()));
            return Ok(());
        }
        let e = match err {
            AgentFailure::Network(_) => e,
            AgentFailure::BadLink(_) | AgentFailure::Other(_) => fail(Code::BadLink, e.detail),
        };
        self.fail_event(RequestKind::Connect, e)
    }

    pub(crate) fn fail_event(&mut self, request: RequestKind, e: EngineError) -> Result<()> {
        self.emit(Event::Failed {
            request,
            code: e.code,
            detail: e.detail,
        });
        Ok(())
    }
}
