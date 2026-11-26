//! Channel types carried in group profiles, link data and relay events (Types.hs `PublicGroupProfile`, `GroupRelayInvitation`; Protocol.hs `RelayCapabilities`, `GroupRoster`, `PublicGroupData`, `RelayShortLinkData`).
use super::group::MemberIdRole;
use super::group_profile::GroupProfile;
use super::ids::MemberId;
use super::open_enum::open_enum;
use super::profile::Profile;
use serde::{Deserialize, Serialize};
use serde_json::Value;

open_enum!(GroupType { Channel = "channel", Group = "group" });

open_enum!(
    /// `RelayRejectionReason`: why a relay refused to serve a channel again.
    RelayRejectionReason { RejoinRejected = "rejoin_rejected" }
);

/// `PublicGroupProfile`: what makes a group profile a channel's. `public_group_id` is the
/// channel's identity, `sha256` of its root key, as padded base64url.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicGroupProfile {
    pub group_type: GroupType,
    pub group_link: String,
    pub public_group_id: String,
    /// `PublicGroupAccess`, kept as sent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_group_access: Option<Value>,
}

/// `RelayCapabilities`: an absent object reads as the default, which encodes as `{}`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayCapabilities {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub web_domain: Option<String>,
}

/// `GroupRelayInvitation`: an owner asks a relay to serve its channel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupRelayInvitation {
    pub from_member: MemberIdRole,
    pub from_member_profile: Profile,
    pub relay_member_id: MemberId,
    pub group_link: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub public_group_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from_member_key: Option<String>,
}

/// `InlineFileInvitation`: the size and SHA-512 digest (padded base64url) of the roster blob that
/// follows as `F` chunks.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InlineFileInvitation {
    pub file_size: u64,
    pub file_digest: String,
}

/// `PublicGroupData`: what a channel's link says beside its profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicGroupData {
    pub public_member_count: i64,
}

/// `RelayShortLinkData`: the user data of a relay's link for one channel.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayShortLinkData {
    pub relay_profile: Profile,
}

impl GroupProfile {
    /// The channel part of the profile, when it is a channel's and reads as one.
    #[must_use]
    pub fn public_group_profile(&self) -> Option<PublicGroupProfile> {
        serde_json::from_value(self.public_group.clone()?).ok()
    }
}
