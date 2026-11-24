//! SetTimer: the user turns disappearing messages on or off for a friend or a group (Commands.hs `SetContactTimedMessages` → `updateContactPrefs`; `SetGroupTimedMessages` → `runUpdateGroupProfile`).

use crate::types::model::{ChatNotice, ChatRef};

use super::rules::{group_ttl, my_pref};
use crate::chat::group_prefs::{FeatureEnabled, GroupFeature};
use crate::chat::preferences::{FeatureAllowed, TimedPref};
use crate::chat::{ChatEvent, ChatMessage};
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::machine::Engine;
use crate::limits::ROLE_TO_EDIT_GROUP;

/// `SetGroupTimedMessages` off: the group keeps a day as its timer for next time.
const GROUP_OFF_TTL: i64 = 86_400;

impl<A: AgentPort> Engine<A> {
    pub(crate) fn set_timer(&mut self, chat: ChatRef, ttl: Option<i64>) -> Result<()> {
        if ttl.is_some_and(|t| t <= 0) {
            return Err(fail(
                Code::RequestFailed,
                "the timer must be at least a second",
            ));
        }
        match chat {
            ChatRef::Direct(friend) => self.set_friend_timer(friend, ttl),
            ChatRef::Group(group) => self.set_group_timer(group, ttl),
        }
    }

    /// On: allowed with this timer (`TMEEnableSetTTL`). Off: not allowed, which turns it off
    /// for both sides, keeping the timer (`TMEDisableKeepTTL`). Their side is told only when
    /// the profile they see changes.
    fn set_friend_timer(&mut self, friend: i64, ttl: Option<i64>) -> Result<()> {
        let f = self.friend(friend)?;
        let pref = match ttl {
            Some(ttl) => TimedPref {
                allow: FeatureAllowed::Yes,
                ttl: Some(ttl),
            },
            None => TimedPref {
                allow: FeatureAllowed::No,
                ttl: my_pref(self.state.user.prefs.as_ref(), &f).ttl,
            },
        };
        let mut prefs = f.my_prefs.clone().unwrap_or_default();
        prefs.timed_messages = Some(pref);
        self.update_friend_prefs(friend, Some(prefs))
    }

    /// The group's preference, sent with its profile in `x.grp.info`.
    fn set_group_timer(&mut self, group: i64, ttl: Option<i64>) -> Result<()> {
        let mut g = self.group(group)?;
        Self::assert_my_role(&g, &ROLE_TO_EDIT_GROUP)?;
        let mut feature = GroupFeature::new(match ttl {
            Some(_) => FeatureEnabled::On,
            None => FeatureEnabled::Off,
        });
        feature.ttl = Some(ttl.unwrap_or(GROUP_OFF_TTL));
        let before = group_ttl(&g);
        let prefs = g
            .profile
            .group_preferences
            .get_or_insert_with(Default::default);
        if prefs.timed_messages.as_ref() == Some(&feature) {
            return Ok(());
        }
        prefs.timed_messages = Some(feature);
        let msg = ChatMessage::new(ChatEvent::GrpInfo(g.profile.clone()))?;
        let now = group_ttl(&g);
        self.save_group(g)?;
        self.send_to_group(group, &msg, None)?;
        if now.flatten() != before.flatten() {
            self.group_notice(group, None, ChatNotice::Timer(now.flatten()))?;
        }
        self.state.chats_dirty = true;
        Ok(())
    }
}
