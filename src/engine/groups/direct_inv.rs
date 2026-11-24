//! A group member's invitation to talk directly, `x.grp.direct.inv`: listed as a request until the user accepts it (Subscriber.hs `xGrpDirectInv`).

use crate::chat::container::MsgScope;
use crate::types::model::ChatNotice;

use crate::agent::links::{ConnectionLink, ConnectionRequestUri};
use crate::chat::content::MsgContent;
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::machine::Engine;
use crate::engine::records::{MemberInvite, RequestRecord, keys};

use super::inbound::GroupCtx;
use super::msg_new::member_can_send;
use super::role::direct_messages_allowed;

/// `connReq` is a `ConnReqInvitation`: a full one-time link, never an address or a short link.
fn is_invitation(link: &str) -> bool {
    matches!(
        ConnectionLink::parse(link),
        Ok(ConnectionLink::Full(
            ConnectionRequestUri::Invitation { .. }
        ))
    )
}

impl<A: AgentPort> Engine<A> {
    /// Only over the member's own connection: a forward of it is never acted on.
    pub(crate) fn on_grp_direct_inv(
        &mut self,
        ctx: GroupCtx,
        link: String,
        content: Option<&MsgContent>,
        scope: Option<&MsgScope>,
    ) -> Result<()> {
        if ctx.forwarded {
            return Err(fail(Code::RequestFailed, "x.grp.direct.inv: forwarded"));
        }
        let g = self.group(ctx.group)?;
        let m = self.member(ctx.group, ctx.sender)?;
        member_can_send(&m, scope)?;
        if !direct_messages_allowed(&g, &m.role) {
            return Err(fail(
                Code::RequestFailed,
                "x.grp.direct.inv: direct messages not allowed",
            ));
        }
        if m.hidden() {
            self.log.line(format_args!(
                "x.grp.direct.inv: member is blocked (ignoring)"
            ));
            return Ok(());
        }
        if !is_invitation(&link) {
            return Err(fail(
                Code::RequestFailed,
                "x.grp.direct.inv: not a one-time invitation link",
            ));
        }
        let invite = MemberInvite {
            group: ctx.group,
            member: m.id,
            link,
            text: content.map(|c| c.text().to_owned()),
        };
        // A member who invites again replaces the earlier invitation and keeps its place.
        let earlier = self.state.address.requests.values().find(|r| {
            r.member
                .as_ref()
                .is_some_and(|i| (i.group, i.member) == (ctx.group, m.id))
        });
        let id = match earlier {
            Some(r) => r.id,
            None => {
                self.make_room()?;
                self.store.next_id(keys::REQUEST_SEQ)?
            }
        };
        let rec = RequestRecord {
            id,
            invitation: Vec::new(),
            name: m.profile.display_name.clone(),
            about: m.profile.short_descr.clone().unwrap_or_default(),
            image: m.profile.image.as_ref().map(|i| i.0.clone()),
            req_id: None,
            since: self.clock.wall_ms(),
            member: Some(invite),
        };
        self.put(&keys::request(id), &rec)?;
        self.state.address.requests.insert(id, rec);
        self.state.chats_dirty = true;
        self.group_notice(ctx.group, Some(m.id), ChatNotice::DirectInvitation)
    }
}
