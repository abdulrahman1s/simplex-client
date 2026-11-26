//! Joining a channel: its link read and checked, the owners it names kept with their keys, and each relay's link read next (Commands.hs `APIPrepareGroup`, `APIConnectPreparedGroup`, `updatePreparedRelayedGroup`, `createLinkOwnerMember`).

use crate::agent::connection::LinkLookup;
use crate::agent::short_link::{UserLinkData, decode_short_link_data};
use crate::chat::channel::{GroupType, PublicGroupData};
use crate::chat::group::{GroupMemberRole, MemberInfo};
use crate::chat::group_profile::GroupProfile;
use crate::chat::ids::{MemberId, b64_decode, b64_encode};
use crate::chat::profile::Profile;
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::machine::Engine;
use crate::engine::records::{
    ChannelRecord, GroupRecord, Lookup, MemberCategory, MemberStatus, keys,
};

use super::keys::new_pair;

/// `GroupShortLinkData`.
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ChannelLinkData {
    group_profile: GroupProfile,
    #[serde(default)]
    public_group_data: Option<PublicGroupData>,
}

fn bad(why: &str) -> crate::engine::error::EngineError {
    fail(Code::BadLink, why.to_owned())
}

/// The profile, checked to be a channel's whose id is the link's (`linkEntityId`).
fn channel_profile(data: &LinkLookup) -> Result<(GroupProfile, Vec<u8>, Option<i64>)> {
    let link: ChannelLinkData = decode_short_link_data(&UserLinkData(data.user_data.clone()))
        .ok()
        .flatten()
        .ok_or_else(|| bad("channel link without group data"))?;
    let pg = link
        .group_profile
        .public_group_profile()
        .ok_or_else(|| bad("not a channel's link"))?;
    let id = b64_decode(&pg.public_group_id).map_err(|_| bad("bad publicGroupId"))?;
    if pg.group_type != GroupType::Channel || data.entity_id.as_deref() != Some(id.as_slice()) {
        return Err(bad("the link's entity ID is not the channel's"));
    }
    let count = link.public_group_data.map(|d| d.public_member_count);
    Ok((link.group_profile, id, count))
}

impl<A: AgentPort> Engine<A> {
    /// `connect` with a channel's link. A channel already joined only reads the list again.
    pub(crate) fn join_channel(&mut self, link: &str) -> Result<()> {
        let known = self
            .state
            .groups
            .values()
            .any(|g| g.channel.as_ref().is_some_and(|c| c.link == link) && !g.status.gone());
        if known {
            self.state.chats_dirty = true;
            return Ok(());
        }
        self.look_up(
            link,
            Lookup::Channel {
                link: link.to_owned(),
            },
        )
    }

    pub(super) fn on_channel_link(&mut self, link: &str, data: LinkLookup) -> Result<()> {
        let (profile, public_group_id, member_count) = channel_profile(&data)?;
        if data.relays.is_empty() {
            return Err(bad("the channel has no relays"));
        }
        let (member_private, _) = new_pair()?;
        let id = self.store.next_id(keys::GROUP_SEQ)?;
        self.save_group(GroupRecord {
            id,
            profile,
            my_member_id: MemberId::mint()?,
            // The relay's `x.grp.link.inv` says the role; a subscriber is an observer until then.
            my_role: GroupMemberRole::Observer,
            status: MemberStatus::Accepted,
            host: None,
            invitation: None,
            invited_by_friend: None,
            since: self.clock.wall_ms(),
            muted: false,
            read_up_to: 0,
            unread: 0,
            link: None,
            link_conn: None,
            local: crate::engine::records::ChatLocal::default(),
            channel: Some(ChannelRecord {
                public_group_id,
                link: link.to_owned(),
                root_key: data.root_key.to_der(),
                root_private: None,
                member_private,
                relays: Vec::new(),
                roster_version: None,
                link_conn: None,
                member_count,
                publishing: Vec::new(),
                seen: std::collections::VecDeque::new(),
            }),
        })?;
        for o in &data.owners {
            self.link_owner(id, &o.owner_id, &b64_encode(&o.owner_key.to_der()))?;
        }
        for relay in &data.relays {
            let m = self.new_relay_member(id, MemberCategory::Host, Some(&relay.to_uri()))?;
            self.look_up(
                &relay.to_uri(),
                Lookup::RelayJoin {
                    group: id,
                    member: m,
                },
            )?;
        }
        self.state.chats_dirty = true;
        Ok(())
    }

    /// `createLinkOwnerMember`: an owner the link names, trusted with its key, not connected.
    fn link_owner(&mut self, group: i64, owner_id: &[u8], key: &str) -> Result<()> {
        let info = MemberInfo {
            member_id: MemberId(owner_id.to_vec()),
            member_role: GroupMemberRole::Owner,
            v: None,
            profile: Profile::new("owner", ""),
            member_key: Some(key.to_owned()),
        };
        self.new_member(
            group,
            &info,
            MemberCategory::Pre,
            MemberStatus::Unknown,
            None,
        )?;
        Ok(())
    }

    /// A relay member before its link is read; the link names its id and key.
    pub(super) fn new_relay_member(
        &mut self,
        group: i64,
        category: MemberCategory,
        link: Option<&str>,
    ) -> Result<i64> {
        let info = MemberInfo {
            member_id: MemberId::mint()?,
            member_role: GroupMemberRole::Relay,
            v: None,
            profile: Profile::new("relay", ""),
            member_key: None,
        };
        let mut m = self.new_member(group, &info, category, MemberStatus::Invited, None)?;
        m.relay_link = link.map(str::to_owned);
        let id = m.id;
        self.save_member(m)?;
        Ok(id)
    }
}
