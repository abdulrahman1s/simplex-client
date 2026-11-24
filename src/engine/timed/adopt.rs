//! A friend's profile as it arrives, and the user's timer for them following theirs (Subscriber.hs `processContactProfileUpdate` `ctUserPrefs'`).

use crate::types::model::ChatFeature;

use crate::chat::preferences::{Preferences, TimedPref};
use crate::chat::profile::Profile;
use crate::engine::prefs::merge::user_allow;
use crate::engine::records::FriendRecord;

/// Their profile's timer replaces the user's own for them, so both sides count the same; the
/// user's "allow" stays. Nothing is sent back: only the user's own change sends `x.info`.
fn adopt_timer(user: Option<&Preferences>, f: &mut FriendRecord, p: &Profile) {
    let theirs = p.preferences.as_ref();
    let rcv_ttl = theirs.and_then(|p| p.timed_messages.as_ref()?.ttl);
    let mine = f.my_prefs.as_ref().and_then(|p| p.timed_messages.clone());
    if rcv_ttl == mine.as_ref().and_then(|t| t.ttl) {
        return;
    }
    let timed = match mine {
        Some(mut t) => {
            t.ttl = rcv_ttl;
            t
        }
        // No override and no user-wide timer: only a timer is worth an override, with the
        // user's own "allow" (`userDefault`).
        None => match rcv_ttl {
            Some(ttl) => TimedPref {
                allow: user_allow(user, None, ChatFeature::TimedMessages),
                ttl: Some(ttl),
            },
            None => return,
        },
    };
    f.my_prefs
        .get_or_insert_with(Preferences::default)
        .timed_messages = Some(timed);
}

pub(crate) fn apply_profile(user: Option<&Preferences>, f: &mut FriendRecord, p: &Profile) {
    let about = p.short_descr.clone().unwrap_or_default();
    let image = p.image.as_ref().map(|i| i.0.clone());
    // Upstream takes their timer only from a profile whose content changed, so one that
    // comes again does not undo the user's own change since.
    let changed = f.name != p.display_name
        || f.about != about
        || f.image != image
        || f.prefs != p.preferences
        || f.address != p.contact_link;
    f.name.clone_from(&p.display_name);
    f.about = about;
    f.image = image;
    f.prefs.clone_from(&p.preferences);
    f.address.clone_from(&p.contact_link);
    if changed {
        adopt_timer(user, f, p);
    }
}
