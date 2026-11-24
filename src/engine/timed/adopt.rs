//! A friend's profile as it arrives, and the user's timer for them following theirs (Subscriber.hs `processContactProfileUpdate` `ctUserPrefs'`).

use super::rules::default_pref;
use crate::chat::profile::Profile;
use crate::engine::records::FriendRecord;

/// Their profile's timer replaces the user's own for them, so both sides count the same; the
/// user's "allow" stays. Nothing is sent back: only the user's own change sends `x.info`.
fn adopt_timer(f: &mut FriendRecord, p: &Profile) {
    let theirs = p
        .preferences
        .as_ref()
        .and_then(|p| p.timed_messages.as_ref());
    let rcv_ttl = theirs.and_then(|t| t.ttl);
    let user_ttl = f.my_timer.as_ref().and_then(|t| t.ttl);
    if rcv_ttl == user_ttl {
        return;
    }
    f.my_timer = match f.my_timer.take() {
        Some(mut mine) => {
            mine.ttl = rcv_ttl;
            Some(mine)
        }
        // The default has no timer, so only a timer is worth keeping as the user's own.
        None => rcv_ttl.map(|ttl| {
            let mut mine = default_pref();
            mine.ttl = Some(ttl);
            mine
        }),
    };
}

pub(crate) fn apply_profile(f: &mut FriendRecord, p: &Profile) {
    let about = p.short_descr.clone().unwrap_or_default();
    let image = p.image.as_ref().map(|i| i.0.clone());
    // Upstream takes their timer only from a profile whose content changed, so one that
    // comes again does not undo the user's own change since.
    let changed = f.name != p.display_name
        || f.about != about
        || f.image != image
        || f.prefs != p.preferences;
    f.name.clone_from(&p.display_name);
    f.about = about;
    f.image = image;
    f.prefs.clone_from(&p.preferences);
    if changed {
        adopt_timer(f, p);
    }
}
