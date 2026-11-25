//! SetPrefs: the user's preferences, for every friend or for one (Commands.hs `APIUpdateProfile` with preferences; `APISetContactPrefs` → `updateContactPrefs`).

use crate::types::model::{ChatPrefs, MessageAuthor};

use super::wire::with;
use crate::chat::preferences::Preferences;
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::machine::Engine;
use crate::engine::profile::sent_profile_to;
use crate::engine::timed::rules::friend_ttl;

impl<A: AgentPort> Engine<A> {
    /// `friend: None` sets the user's own, sent to each friend whose profile it changes.
    pub(crate) fn set_prefs(&mut self, friend: Option<i64>, set: &ChatPrefs) -> Result<()> {
        let Some(friend) = friend else {
            let mut user = self.state.user.clone();
            user.prefs = with(user.prefs.as_ref(), set);
            return self.update_profile(user);
        };
        let f = self.friend(friend)?;
        let prefs = with(f.my_prefs.as_ref(), set);
        self.update_friend_prefs(friend, prefs)
    }

    /// `updateContactPrefs`: the user's overrides for one friend. They are told only when the
    /// profile they are sent changes; the conversation says so when the timer that applies did.
    pub(crate) fn update_friend_prefs(
        &mut self,
        friend: i64,
        prefs: Option<Preferences>,
    ) -> Result<()> {
        let mut f = self.friend(friend)?;
        if f.my_prefs == prefs {
            return Ok(());
        }
        if !f.reachable() {
            return Err(fail(Code::RequestFailed, "not connected"));
        }
        let user = self.state.user.clone();
        let before = friend_ttl(user.prefs.as_ref(), &f);
        let seen = sent_profile_to(&user, &f);
        f.my_prefs = prefs;
        let changed = sent_profile_to(&user, &f) != seen;
        self.save_friend(f)?;
        if changed {
            self.send_profile(friend)?;
        }
        self.timer_notice(friend, before, MessageAuthor::Me)?;
        self.state.chats_dirty = true;
        Ok(())
    }
}
