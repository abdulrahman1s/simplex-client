//! Whether a chat's messages disappear and after how long (Messages.hs `contactTimedTTL`, `groupTimedTTL`, `rcvCITimed_`; Internal.hs `sndCITimed_`).

use crate::types::model::{ChatFeature, Timed};

use crate::chat::group_prefs::FeatureEnabled;
use crate::chat::preferences::{Preferences, TimedPref};
use crate::engine::prefs::merge::{allow_of, default_allow, pref_enabled, user_allow};
use crate::engine::records::{FriendRecord, GroupRecord};

/// The user's preference for this friend: the "allow" from their override, the user's own or
/// the default; the timer only from the override (no user-wide timer is ever set here).
#[must_use]
pub fn my_pref(user: Option<&Preferences>, f: &FriendRecord) -> TimedPref {
    // Incognito, the friend was sent the defaults, not the user's own (`mergeUserChatPrefs'`).
    let user = user.filter(|_| f.incognito.is_none());
    let mine = f.my_prefs.as_ref();
    TimedPref {
        allow: user_allow(user, mine, ChatFeature::TimedMessages),
        ttl: mine.and_then(|p| p.timed_messages.as_ref()?.ttl),
    }
}

/// `contactTimedTTL`: `Some` when both sides allow it (`prefEnabled False`: any "no" turns it
/// off for both), holding the user's timer, which is `None` while none is set.
#[must_use]
pub fn friend_ttl(user: Option<&Preferences>, f: &FriendRecord) -> Option<Option<i64>> {
    let mine = my_pref(user, f);
    let theirs = f
        .prefs
        .as_ref()
        .and_then(|p| allow_of(p, ChatFeature::TimedMessages));
    let on = pref_enabled(
        ChatFeature::TimedMessages,
        mine.allow,
        theirs.unwrap_or(default_allow(ChatFeature::TimedMessages)),
    );
    (on.for_user && on.for_contact).then_some(mine.ttl)
}

/// `groupTimedTTL`: `Some` when the group has it on (off by default), with the group's timer.
#[must_use]
pub fn group_ttl(g: &GroupRecord) -> Option<Option<i64>> {
    let prefs = g.profile.group_preferences.as_ref();
    let feature = prefs.and_then(|p| p.timed_messages.as_ref())?;
    (feature.enable == FeatureEnabled::On).then_some(feature.ttl)
}

/// When an item of `ttl` seconds made at `now` goes; a negative `ttl` goes at once.
#[must_use]
pub fn deadline(now: i64, ttl: i64) -> i64 {
    now.saturating_add(ttl.max(0).saturating_mul(1000))
}

/// `sndCITimed_` (never live here): the user's item counts down from sending.
#[must_use]
pub fn sent(chat_ttl: Option<Option<i64>>, now: i64) -> Option<Timed> {
    let ttl = chat_ttl.flatten()?;
    Some(Timed {
        ttl,
        delete_at: Some(deadline(now, ttl)),
    })
}

/// `rcvCITimed_`: the item's own ttl when the chat allows timed messages at all; its count
/// starts when it is read.
#[must_use]
pub fn received(chat_ttl: Option<Option<i64>>, item_ttl: Option<i64>) -> Option<Timed> {
    chat_ttl?;
    Some(Timed {
        ttl: item_ttl?,
        delete_at: None,
    })
}
