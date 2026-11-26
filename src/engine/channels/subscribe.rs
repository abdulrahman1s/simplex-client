//! A subscriber and one relay: the relay's link joined with a signed `x.member`, its `x.grp.link.inv` answered with `x.info`, and the first relay connected tells the user the channel is joined (Commands.hs `connectToRelay`, `encodeXMemberConnInfo`; Subscriber.hs `CONF` from `GCHostMember`, `CON`).

use crate::agent::connection::LinkLookup;
use crate::agent::links::ConnectionLink;
use crate::agent::short_link::{UserLinkData, decode_short_link_data};
use crate::chat::channel::RelayShortLinkData;
use crate::chat::compress::compress_to_limit;
use crate::chat::element::encode_signed_element;
use crate::chat::ids::{MemberId, b64_encode};
use crate::chat::limits::MAX_CONN_INFO_LENGTH;
use crate::chat::{ChatEvent, ChatMessage};
use crate::engine::agent_port::{AgentPort, ConnId};
use crate::engine::error::{Code, Result, fail};
use crate::engine::groups::peer_range;
use crate::engine::machine::Engine;
use crate::engine::records::{ConnKind, ConnRecord, GroupRecord, MemberStatus};
use crate::types::model::ChatNotice;

use super::keys::{binding, public_b64, sign};

fn dropped(why: &str) -> crate::engine::error::EngineError {
    fail(Code::RequestFailed, why.to_owned())
}

impl<A: AgentPort> Engine<A> {
    /// The user's event as signed conn info: one `/` element, compressed to fit (`encodeSignedConnInfo`).
    pub(crate) fn signed_conn_info(&self, g: &GroupRecord, event: ChatEvent) -> Result<Vec<u8>> {
        let c = g.channel.as_ref().ok_or_else(|| dropped("not a channel"))?;
        let body = ChatMessage::without_id(event).to_json()?;
        let signed = sign(
            &c.member_private,
            &binding(&c.public_group_id, &g.my_member_id)?,
            body,
        )?;
        Ok(compress_to_limit(
            &encode_signed_element(&signed)?,
            MAX_CONN_INFO_LENGTH,
        )?)
    }

    /// `CFGetRelayDataJoin`: the relay's id and key are its link's; the join proves the
    /// user's member key and names the relay, so a sibling relay can't replay it (`viaRelay`).
    pub(super) fn on_relay_link_to_join(
        &mut self,
        group: i64,
        member: i64,
        data: LinkLookup,
    ) -> Result<()> {
        let relay: RelayShortLinkData = decode_short_link_data(&UserLinkData(data.user_data))
            .ok()
            .flatten()
            .ok_or_else(|| dropped("relay link: no relay link data"))?;
        let entity = data
            .entity_id
            .ok_or_else(|| dropped("relay link: no entity id"))?;
        let mut m = self.member(group, member)?;
        m.member_id = MemberId(entity);
        m.key = Some(b64_encode(&data.root_key.to_der()));
        m.profile = relay.relay_profile;
        let g = self.group(group)?;
        let c = g.channel.as_ref().ok_or_else(|| dropped("not a channel"))?;
        let info = self.signed_conn_info(
            &g,
            ChatEvent::Member {
                profile: self.handshake_profile(&g),
                new_member_id: g.my_member_id.clone(),
                new_member_key: public_b64(&c.member_private)?,
                via_relay: Some(m.member_id.clone()),
            },
        )?;
        // Group connections are PQ off (Subscriber.hs `CFGetRelayDataJoin`: `PQSupportOff`).
        let link = ConnectionLink::Full(data.conn_req);
        let conn = self.agent.join(&mut self.store, &link, info, false)?;
        m.conn = Some(conn);
        m.status = MemberStatus::Accepted;
        self.save_member(m)?;
        self.save_conn(ConnRecord {
            conn,
            kind: ConnKind::Member { group, member },
        })?;
        Ok(())
    }

    /// The relay's CONF: `x.grp.link.inv` for this very channel says the user's role; the
    /// answer is the user's profile and member key, signed.
    pub(super) fn on_subscriber_conf(
        &mut self,
        group: i64,
        member: i64,
        conn: &ConnId,
        msg: ChatMessage,
    ) -> Result<()> {
        let range = peer_range(&msg);
        let ChatEvent::GrpLinkInv(inv) = msg.event else {
            return Err(dropped("CONF from a relay must have x.grp.link.inv"));
        };
        let mut g = self.group(group)?;
        let ours = g.profile.public_group_profile().map(|p| p.public_group_id);
        let theirs = inv
            .group_profile
            .public_group_profile()
            .map(|p| p.public_group_id);
        if ours.is_none() || ours != theirs {
            return Err(dropped("x.grp.link.inv: publicGroupId mismatch"));
        }
        if inv.invited_member.member_id != g.my_member_id {
            return Err(dropped("x.grp.link.inv: not the user's member id"));
        }
        g.my_role = inv.invited_member.member_role;
        g.profile = inv.group_profile;
        self.save_group(g.clone())?;
        let c = g.channel.as_ref().ok_or_else(|| dropped("not a channel"))?;
        let info = self.signed_conn_info(
            &g,
            ChatEvent::Info {
                profile: self.handshake_profile(&g),
                member_key: Some(public_b64(&c.member_private)?),
            },
        )?;
        self.agent.allow(&mut self.store, conn, info)?;
        let mut m = self.member(group, member)?;
        m.chat_v = Some(range);
        self.save_member(m)?;
        Ok(())
    }

    /// The first relay connected joins the user: "not end-to-end encrypted" (`e2eInfoGroup`
    /// of a public group), then joined. Later relays only connect.
    pub(super) fn on_subscriber_con(&mut self, group: i64, member: i64) -> Result<()> {
        let mut m = self.member(group, member)?;
        m.status = MemberStatus::Connected;
        self.save_member(m)?;
        // Each relay's connection shows on the group (`channel.relays`).
        self.state.chats_dirty = true;
        let mut g = self.group(group)?;
        if g.status == MemberStatus::Connected {
            return Ok(());
        }
        g.status = MemberStatus::Connected;
        self.save_group(g)?;
        self.group_notice(group, None, ChatNotice::NotEncrypted)?;
        self.group_notice(group, None, ChatNotice::YouJoined)?;
        self.state.chats_dirty = true;
        Ok(())
    }
}
