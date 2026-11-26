//! The channel's link data published again with the relays that may serve it, and the relays it names made active once the server stored it (Internal.hs `setGroupLinkDataAsync`, `getPublishableGroupRelays`; Subscriber.hs `LINK`).

use crate::agent::links::ConnShortLink;
use crate::agent::short_link::{OwnerAuth, encode_short_link_data};
use crate::chat::channel::PublicGroupData;
use crate::engine::agent_port::{AgentPort, ConnId};
use crate::engine::error::{Code, Result, fail};
use crate::engine::machine::Engine;
use crate::engine::records::{GroupRecord, RelayStatus};

use super::create::ChannelLinkData;
use super::keys::private;

fn failed(what: &str) -> crate::engine::error::EngineError {
    fail(Code::RequestFailed, format!("channel link: {what}"))
}

/// The owner's `OwnerAuth`, signed by the root key again (Ed25519 signs deterministically).
fn owner_auth(g: &GroupRecord) -> Result<OwnerAuth> {
    let c = g.channel.as_ref().ok_or_else(|| failed("not a channel"))?;
    let root = private(
        c.root_private
            .as_deref()
            .ok_or_else(|| failed("not the owner"))?,
    )?;
    let key = private(&c.member_private)?
        .public()
        .map_err(|_| failed("bad member key"))?;
    let sig = root
        .sign(&[g.my_member_id.0.as_slice(), &key.to_der()].concat())
        .map_err(|_| failed("signing failed"))?;
    Ok(OwnerAuth {
        owner_id: g.my_member_id.0.clone(),
        owner_key: key,
        auth_owner_sig: sig,
    })
}

impl<A: AgentPort> Engine<A> {
    /// `getPublishableGroupRelays`: relays that took the roster, and an older relay accepted.
    pub(super) fn publish_channel(&mut self, group: i64) -> Result<()> {
        let mut g = self.group(group)?;
        let owners = vec![owner_auth(&g)?];
        let old: Vec<i64> = (self.state.members_of(group))
            .filter(|m| {
                m.chat_v
                    .is_some_and(|v| v.max < super::roster::ROSTER_VERSION)
            })
            .map(|m| m.id)
            .collect();
        let c = g.channel.as_mut().ok_or_else(|| failed("not a channel"))?;
        let conn = c.link_conn.ok_or_else(|| failed("no link connection"))?;
        let links: Vec<String> = c
            .relays
            .iter()
            .filter(|r| match r.status {
                RelayStatus::AcknowledgedRoster | RelayStatus::Active => true,
                RelayStatus::Accepted => old.contains(&r.member),
                RelayStatus::New
                | RelayStatus::Invited
                | RelayStatus::Inactive
                | RelayStatus::Rejected => false,
            })
            .filter_map(|r| r.link.clone())
            .collect();
        let relays = links
            .iter()
            .map(|l| ConnShortLink::parse(l).map_err(|_| failed("bad relay link")))
            .collect::<Result<Vec<_>>>()?;
        let count = c.member_count.unwrap_or(1);
        c.publishing.clone_from(&links);
        let data = ChannelLinkData {
            group_profile: &g.profile,
            public_group_data: PublicGroupData {
                public_member_count: count,
            },
        };
        let user_data = encode_short_link_data(&data)
            .map_err(|_| failed("too large"))?
            .0;
        self.save_group(g)?;
        self.agent
            .set_link_data(&mut self.store, &conn, owners, relays, user_data)?;
        Ok(())
    }

    /// `LINK`: the relays the stored data names are active.
    pub(crate) fn on_channel_link_updated(&mut self, conn: &ConnId) -> Result<()> {
        let Some(group) = self
            .state
            .groups
            .values()
            .find(|g| {
                g.channel
                    .as_ref()
                    .is_some_and(|c| c.link_conn == Some(*conn))
            })
            .map(|g| g.id)
        else {
            return Ok(());
        };
        let g = self.group(group)?;
        let Some(c) = g.channel.as_ref() else {
            return Ok(());
        };
        let up: Vec<i64> = c
            .relays
            .iter()
            .filter(|r| r.link.as_ref().is_some_and(|l| c.publishing.contains(l)))
            .filter(|r| {
                matches!(
                    r.status,
                    RelayStatus::AcknowledgedRoster | RelayStatus::Accepted
                )
            })
            .map(|r| r.member)
            .collect();
        for member in up {
            self.set_relay_status(group, member, RelayStatus::Active)?;
        }
        self.state.chats_dirty = true;
        Ok(())
    }
}
