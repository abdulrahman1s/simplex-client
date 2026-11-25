//! What is on between the user and a friend, from both sides' preferences (Types.hs `contactUserPreferences`; Types/Preferences.hs `featureAllowed`).

use std::collections::BTreeMap;

use crate::types::model::{ChatFeature, PrefEnabled};

use super::merge::{default_allow, pref_enabled, user_allow};
use crate::engine::agent_port::AgentPort;
use crate::engine::machine::Engine;
use crate::engine::records::FriendRecord;

impl<A: AgentPort> Engine<A> {
    /// One feature with this friend: the user's side against theirs, each missing one the default.
    #[must_use]
    pub(crate) fn enabled(&self, f: &FriendRecord, feature: ChatFeature) -> PrefEnabled {
        // Incognito, the friend was sent the defaults, not the user's own (`mergeUserChatPrefs'`).
        let user = self
            .state
            .user
            .prefs
            .as_ref()
            .filter(|_| f.incognito.is_none());
        let mine = user_allow(user, f.my_prefs.as_ref(), feature);
        let theirs = (f.prefs.as_ref())
            .and_then(|p| super::merge::allow_of(p, feature))
            .unwrap_or(default_allow(feature));
        pref_enabled(feature, mine, theirs)
    }

    /// Every feature with this friend, for the page.
    #[must_use]
    pub(crate) fn features(&self, f: &FriendRecord) -> BTreeMap<ChatFeature, PrefEnabled> {
        ChatFeature::ALL
            .into_iter()
            .map(|feature| (feature, self.enabled(f, feature)))
            .collect()
    }
}
