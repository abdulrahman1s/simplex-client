//! NewChannel: a root key whose hash is the channel's id, the owner's member key authorised by it, the channel's link made under it, and the relays invited once the link exists (Commands.hs `APINewPublicGroup`, `prepareGroupLink`; Crypto/ShortLink.hs `newOwnerAuth`).

use crate::agent::short_link::{OwnerAuth, encode_short_link_data};
use crate::chat::channel::{GroupType, PublicGroupData, PublicGroupProfile};
use crate::chat::group::GroupMemberRole;
use crate::chat::group_prefs::{FeatureEnabled, GroupFeature, GroupPreferences};
use crate::chat::group_profile::GroupProfile;
use crate::chat::ids::{MemberId, b64_encode};
use crate::crypto::{Alg, KeyPair, sha256};
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::groups::link_data::client_data;
use crate::engine::machine::Engine;
use crate::engine::records::{
    ChannelRecord, ConnKind, ConnRecord, GroupRecord, MemberStatus, keys,
};
use crate::types::model::ChatNotice;

use super::keys::private;

fn key_failed(_: crate::crypto::CryptoError) -> crate::engine::error::EngineError {
    fail(Code::RequestFailed, "channel key: crypto failed")
}

/// `channelProfile`: history on (the relays replay it), support off.
fn channel_profile(name: &str) -> GroupProfile {
    GroupProfile {
        display_name: name.to_owned(),
        full_name: String::new(),
        short_descr: None,
        description: None,
        image: None,
        public_group: None,
        group_preferences: Some(GroupPreferences {
            direct_messages: Some(GroupFeature::new(FeatureEnabled::On)),
            history: Some(GroupFeature::new(FeatureEnabled::On)),
            support: Some(GroupFeature::new(FeatureEnabled::Off)),
            ..GroupPreferences::default()
        }),
        member_admission: None,
        chat_layout: None,
    }
}

/// `GroupShortLinkData` of a channel.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ChannelLinkData<'a> {
    pub group_profile: &'a GroupProfile,
    pub public_group_data: PublicGroupData,
}

impl<A: AgentPort> Engine<A> {
    /// Needs the caller's chat relays (`SetChatRelays`); none is ever chosen for it.
    pub(crate) fn new_channel(&mut self, name: &str) -> Result<()> {
        let name = name.trim();
        if name.is_empty() {
            return Err(fail(Code::RequestFailed, "a channel needs a name"));
        }
        if self.state.settings.chat_relays.is_empty() {
            return Err(fail(
                Code::NotAllowed,
                "no chat relays: set them with SetChatRelays",
            ));
        }
        let root = KeyPair::generate(Alg::Ed25519).map_err(key_failed)?;
        let entity_id = sha256(root.public.as_bytes()).to_vec();
        let my_member_id = MemberId::mint()?;
        let (member_private, _) = super::keys::new_pair()?;
        let member_public = private(&member_private)?.public().map_err(key_failed)?;
        let owner = OwnerAuth {
            owner_id: my_member_id.0.clone(),
            auth_owner_sig: root
                .private
                .sign(&[my_member_id.0.as_slice(), &member_public.to_der()].concat())
                .map_err(key_failed)?,
            owner_key: member_public,
        };
        let base = channel_profile(name);
        let public_group_id = b64_encode(&entity_id);
        let profile_cell = std::rc::Rc::new(std::cell::RefCell::new(None));
        let cell = profile_cell.clone();
        let user_data = Box::new(move |link: &str| {
            let mut p = base;
            p.public_group = serde_json::to_value(PublicGroupProfile {
                group_type: GroupType::Channel,
                group_link: link.to_owned(),
                public_group_id,
                public_group_access: None,
            })
            .ok();
            let data = ChannelLinkData {
                group_profile: &p,
                public_group_data: PublicGroupData {
                    public_member_count: 1,
                },
            };
            let bytes = encode_short_link_data(&data)
                .map(|d| d.0)
                .unwrap_or_default();
            *cell.borrow_mut() = Some((p, link.to_owned()));
            bytes
        });
        let conn = self.agent.create_channel_link(
            &mut self.store,
            &root.private,
            entity_id.clone(),
            vec![owner],
            user_data,
            client_data()?,
        )?;
        let Some((profile, link)) = profile_cell.borrow_mut().take() else {
            return Err(fail(Code::RequestFailed, "channel link: no profile"));
        };
        let id = self.store.next_id(keys::GROUP_SEQ)?;
        self.save_group(GroupRecord {
            id,
            profile,
            my_member_id,
            my_role: GroupMemberRole::Owner,
            status: MemberStatus::Creator,
            host: None,
            invitation: None,
            invited_by_friend: None,
            since: self.clock.wall_ms(),
            muted: false,
            read_up_to: 0,
            unread: 0,
            link: None,
            link_conn: Some(conn),
            local: crate::engine::records::ChatLocal::default(),
            channel: Some(ChannelRecord {
                public_group_id: entity_id,
                link,
                root_key: root.public.to_der(),
                root_private: Some(root.private.to_der().to_vec()),
                member_private,
                relays: Vec::new(),
                roster_version: Some(0),
                link_conn: Some(conn),
                member_count: Some(1),
                publishing: Vec::new(),
                seen: std::collections::VecDeque::new(),
            }),
        })?;
        self.save_conn(ConnRecord {
            conn,
            kind: ConnKind::GroupLink { group: id },
        })?;
        self.group_notice(id, None, ChatNotice::NotEncrypted)?;
        self.state.chats_dirty = true;
        Ok(())
    }
}
