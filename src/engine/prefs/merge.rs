//! The user's preference for a friend, the profile it goes in, and what is on between them (Types.hs `contactUserPreferences`, `userProfileDirect`; Types/Preferences.hs `mergePreferences`, `prefEnabled`).

use crate::types::model::{ChatFeature, PrefEnabled};

use crate::chat::preferences::{AllowPref, FeatureAllowed, Preferences, TimedPref};

/// `defaultChatPrefs`.
#[must_use]
pub const fn default_allow(f: ChatFeature) -> FeatureAllowed {
    match f {
        ChatFeature::FullDelete => FeatureAllowed::No,
        ChatFeature::Files => FeatureAllowed::Always,
        ChatFeature::TimedMessages
        | ChatFeature::Reactions
        | ChatFeature::Voice
        | ChatFeature::Calls => FeatureAllowed::Yes,
    }
}

/// What a set of preferences says of `f`, if anything.
#[must_use]
pub fn allow_of(p: &Preferences, f: ChatFeature) -> Option<FeatureAllowed> {
    let simple = |a: &Option<AllowPref>| a.as_ref().map(|a| a.allow);
    match f {
        ChatFeature::TimedMessages => p.timed_messages.as_ref().map(|t| t.allow),
        ChatFeature::FullDelete => simple(&p.full_delete),
        ChatFeature::Reactions => simple(&p.reactions),
        ChatFeature::Voice => simple(&p.voice),
        ChatFeature::Files => simple(&p.files),
        ChatFeature::Calls => simple(&p.calls),
    }
}

/// The user's side for one friend: their override, else the user's own, else the default
/// (`mergeUserChatPrefs'`, `getPreference`).
#[must_use]
pub fn user_allow(
    user: Option<&Preferences>,
    mine: Option<&Preferences>,
    f: ChatFeature,
) -> FeatureAllowed {
    (mine.and_then(|p| allow_of(p, f)))
        .or_else(|| user.and_then(|p| allow_of(p, f)))
        .unwrap_or(default_allow(f))
}

/// `prefEnabled`: only disappearing messages are symmetric, so a "no" from either side turns
/// them off for both, where `always` against `no` leaves the other features on for one side.
#[must_use]
pub fn pref_enabled(f: ChatFeature, user: FeatureAllowed, contact: FeatureAllowed) -> PrefEnabled {
    use FeatureAllowed::{Always, No};
    let asymmetric = f != ChatFeature::TimedMessages;
    let (for_user, for_contact) = match (user, contact) {
        (Always, No) => (false, asymmetric),
        (No, Always) => (asymmetric, false),
        (_, No) | (No, _) => (false, false),
        _ => (true, true),
    };
    PrefEnabled {
        for_user,
        for_contact,
    }
}

/// The user's preferences in full, as their profile carries them (`fullPreferences`).
#[must_use]
pub fn full(user: Option<&Preferences>) -> Preferences {
    let mut out = Preferences::default_sent();
    let Some(u) = user else { return out };
    let simple = |a: FeatureAllowed| Some(AllowPref { allow: a });
    for f in ChatFeature::ALL {
        let Some(a) = allow_of(u, f) else { continue };
        match f {
            ChatFeature::TimedMessages => {
                out.timed_messages = Some(TimedPref {
                    allow: a,
                    ttl: u.timed_messages.as_ref().and_then(|t| t.ttl),
                });
            }
            ChatFeature::FullDelete => out.full_delete = simple(a),
            ChatFeature::Reactions => out.reactions = simple(a),
            ChatFeature::Voice => out.voice = simple(a),
            ChatFeature::Files => out.files = simple(a),
            ChatFeature::Calls => out.calls = simple(a),
        }
    }
    out
}

/// What one friend is sent (`mergePreferences` without falling back to a user-wide timer):
/// each feature from their override, else the user's own; the timer only from the override.
#[must_use]
pub fn sent_to(user: Option<&Preferences>, mine: Option<&Preferences>) -> Preferences {
    let mut out = full(user);
    let Some(m) = mine else {
        if let Some(t) = out.timed_messages.as_mut() {
            t.ttl = None;
        }
        return out;
    };
    let base = out.clone();
    out = full(Some(&overlay(&base, m)));
    if let Some(t) = out.timed_messages.as_mut() {
        t.ttl = m.timed_messages.as_ref().and_then(|t| t.ttl);
    }
    out
}

/// `over`'s features on top of `base`.
fn overlay(base: &Preferences, over: &Preferences) -> Preferences {
    let mut out = base.clone();
    if over.timed_messages.is_some() {
        out.timed_messages.clone_from(&over.timed_messages);
    }
    let pick = |o: &Option<AllowPref>, b: &Option<AllowPref>| o.clone().or_else(|| b.clone());
    out.full_delete = pick(&over.full_delete, &base.full_delete);
    out.reactions = pick(&over.reactions, &base.reactions);
    out.voice = pick(&over.voice, &base.voice);
    out.files = pick(&over.files, &base.files);
    out.calls = pick(&over.calls, &base.calls);
    out
}
