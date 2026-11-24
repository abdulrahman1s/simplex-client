//! The role and feature checks of the join path (Subscriber.hs `checkHostRole`; Types/Preferences.hs); the order of roles is `rank.rs`.

use crate::chat::group::GroupMemberRole;
use crate::chat::group_prefs::{FeatureEnabled, GroupFeature};
use crate::engine::records::GroupRecord;

pub(crate) use super::rank::at_least;

/// `checkHostRole` (Subscriber.hs:3705): who announces or introduces a member must be an admin
/// and hold at least the role it gives that member.
#[must_use]
pub(crate) const fn may_admit(sender: &GroupMemberRole, given: &GroupMemberRole) -> bool {
    at_least(sender, &GroupMemberRole::Admin) && at_least(sender, given)
}

/// `groupFeatureMemberAllowed SGFDirectMessages` (Types/Preferences.hs): on in the group, and the
/// member holds the role it names, if any. Off by default (`defaultGroupPrefs`).
#[must_use]
pub(crate) fn direct_messages_allowed(g: &GroupRecord, role: &GroupMemberRole) -> bool {
    let prefs = g.profile.group_preferences.as_ref();
    member_allowed(prefs.and_then(|p| p.direct_messages.as_ref()), false, role)
}

/// `groupFeatureMemberAllowed SGFSimplexLinks`: on by default (`defaultGroupPrefs`).
#[must_use]
pub(crate) fn simplex_links_allowed(g: &GroupRecord, role: &GroupMemberRole) -> bool {
    let prefs = g.profile.group_preferences.as_ref();
    member_allowed(prefs.and_then(|p| p.simplex_links.as_ref()), true, role)
}

/// `groupFeatureMemberAllowed'`: a feature the group did not set takes its default.
fn member_allowed(f: Option<&GroupFeature>, default_on: bool, role: &GroupMemberRole) -> bool {
    let Some(f) = f else {
        return default_on;
    };
    let has_role = f
        .role
        .as_deref()
        .is_none_or(|min| at_least(role, &GroupMemberRole::from_wire(min)));
    f.enable == FeatureEnabled::On && has_role
}

/// `groupFeatureAllowed SGFHistory`: off by default.
#[must_use]
pub(crate) fn history_on(g: &GroupRecord) -> bool {
    g.profile
        .group_preferences
        .as_ref()
        .and_then(|p| p.history.as_ref())
        .is_some_and(|h| h.enable == FeatureEnabled::On)
}
