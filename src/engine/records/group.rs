//! A group (Types.hs `GroupInfo`): its profile, the user's own membership and its local settings.

use serde::{Deserialize, Serialize};

use super::ChatLocal;
use crate::chat::group::{GroupInvitation, GroupMemberRole};
use crate::chat::group_profile::GroupProfile;
use crate::chat::ids::MemberId;
use crate::engine::agent_port::ConnId;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct GroupRecord {
    pub id: i64,
    pub profile: GroupProfile,
    /// The user's own member: id, role and status (`GroupInfo.membership`).
    pub my_member_id: MemberId,
    pub my_role: GroupMemberRole,
    pub status: MemberStatus,
    /// The member who invited the user (`GCHostMember`); `None` in a group the user made.
    pub host: Option<i64>,
    /// While the user is invited: the invitation to join with `JoinGroup`, and the friend it came from.
    pub invitation: Option<GroupInvitation>,
    pub invited_by_friend: Option<i64>,
    /// Milliseconds since the epoch.
    pub since: i64,
    pub muted: bool,
    /// Items up to this id are read.
    pub read_up_to: i64,
    pub unread: u32,
    /// The group link (`/g#…`, its short form when the relay keeps one) and its contact connection.
    pub link: Option<String>,
    pub link_conn: Option<ConnId>,
    #[serde(default)]
    pub local: ChatLocal,
    /// A channel's identity, keys and relays; `None` in a classic group (`useRelays` off).
    #[serde(default)]
    pub channel: Option<super::ChannelRecord>,
}

/// `GroupMemberStatus` (Types.hs), for the user's membership and for every member.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberStatus {
    Rejected,
    Removed,
    Left,
    GroupDeleted,
    /// Known only from a message an admin forwarded.
    Unknown,
    Invited,
    PendingApproval,
    PendingReview,
    /// `x.grp.mem.intro` arrived for this pre-member.
    Introduced,
    IntroInvited,
    Accepted,
    /// The host announced it (`x.grp.mem.new`).
    Announced,
    Connected,
    Complete,
    Creator,
}

impl MemberStatus {
    /// `memberCurrent`: still in the group (not gone, and past the invitation stage).
    #[must_use]
    pub const fn current(self) -> bool {
        matches!(
            self,
            Self::Introduced
                | Self::IntroInvited
                | Self::Accepted
                | Self::Announced
                | Self::Connected
                | Self::Complete
                | Self::Creator
        )
    }

    /// `memberActive`: in the group with a working connection, or waiting for approval.
    #[must_use]
    pub const fn active(self) -> bool {
        matches!(
            self,
            Self::PendingApproval
                | Self::PendingReview
                | Self::Connected
                | Self::Complete
                | Self::Creator
        )
    }

    /// `memberRemoved`: gone for good.
    #[must_use]
    pub const fn gone(self) -> bool {
        matches!(
            self,
            Self::Rejected | Self::Removed | Self::Left | Self::GroupDeleted
        )
    }
}
