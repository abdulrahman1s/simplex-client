//! Preferences between the page's `ChatPrefs` and the protocol's `Preferences`.

use crate::types::model::{Allow, ChatFeature, ChatPrefs};

use super::merge::allow_of;
use crate::chat::preferences::{AllowPref, FeatureAllowed, Preferences, TimedPref};

const fn to_allow(a: FeatureAllowed) -> Allow {
    match a {
        FeatureAllowed::Always => Allow::Always,
        FeatureAllowed::Yes => Allow::Yes,
        FeatureAllowed::No => Allow::No,
    }
}

const fn from_allow(a: Allow) -> FeatureAllowed {
    match a {
        Allow::Always => FeatureAllowed::Always,
        Allow::Yes => FeatureAllowed::Yes,
        Allow::No => FeatureAllowed::No,
    }
}

/// What `p` sets, feature by feature.
#[must_use]
pub fn to_wire(p: Option<&Preferences>) -> ChatPrefs {
    let of = |f| p.and_then(|p| allow_of(p, f)).map(to_allow);
    ChatPrefs {
        timed_messages: of(ChatFeature::TimedMessages),
        full_delete: of(ChatFeature::FullDelete),
        reactions: of(ChatFeature::Reactions),
        voice: of(ChatFeature::Voice),
        files: of(ChatFeature::Files),
        calls: of(ChatFeature::Calls),
    }
}

/// `old` with `set`'s features: a missing one is no longer set, except disappearing messages,
/// whose timer and its "allow" stay as they were unless `set` names one (`Request::SetTimer`
/// changes the timer). `None` when nothing is left set.
#[must_use]
pub fn with(old: Option<&Preferences>, set: &ChatPrefs) -> Option<Preferences> {
    let mut p = old.cloned().unwrap_or_default();
    let simple = |a: Option<Allow>| {
        a.map(|a| AllowPref {
            allow: from_allow(a),
        })
    };
    if let Some(a) = set.timed_messages {
        let ttl = p.timed_messages.as_ref().and_then(|t| t.ttl);
        p.timed_messages = Some(TimedPref {
            allow: from_allow(a),
            ttl,
        });
    }
    p.full_delete = simple(set.full_delete);
    p.reactions = simple(set.reactions);
    p.voice = simple(set.voice);
    p.files = simple(set.files);
    p.calls = simple(set.calls);
    (p != Preferences::default()).then_some(p)
}
