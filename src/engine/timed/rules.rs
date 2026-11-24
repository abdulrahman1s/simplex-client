//! Whether a chat's messages disappear and after how long (Messages.hs `contactTimedTTL`, `groupTimedTTL`, `rcvCITimed_`; Internal.hs `sndCITimed_`).

use crate::types::model::Timed;

use crate::chat::group_prefs::FeatureEnabled;
use crate::chat::preferences::{FeatureAllowed, TimedPref};
use crate::engine::records::{FriendRecord, GroupRecord};

/// `defaultChatPrefs`: allowed, with no timer.
#[must_use]
pub fn default_pref() -> TimedPref {
    TimedPref {
        allow: FeatureAllowed::Yes,
        ttl: None,
    }
}

/// The user's preference for this friend: their own setting, or the default.
#[must_use]
pub fn my_pref(f: &FriendRecord) -> TimedPref {
    f.my_timer.clone().unwrap_or_else(default_pref)
}

fn their_allow(f: &FriendRecord) -> FeatureAllowed {
    let pref = f.prefs.as_ref().and_then(|p| p.timed_messages.as_ref());
    pref.map_or(FeatureAllowed::Yes, |p| p.allow)
}

/// `contactTimedTTL`: `Some` when both sides allow it (`prefEnabled False`: any "no" turns it
/// off for both), holding the user's timer, which is `None` while none is set.
#[must_use]
pub fn friend_ttl(f: &FriendRecord) -> Option<Option<i64>> {
    let mine = my_pref(f);
    let on = mine.allow != FeatureAllowed::No && their_allow(f) != FeatureAllowed::No;
    on.then_some(mine.ttl)
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
