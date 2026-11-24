//! SetGroupPrefs: an owner turns group features on or off, sent with the profile in `x.grp.info` (Commands.hs `APIUpdateGroupProfile`).

use crate::types::model::{GroupPref, GroupPrefs};

use super::convert::to_chat;
use crate::chat::group_prefs::{FeatureEnabled, GroupFeature};
use crate::chat::{ChatEvent, ChatMessage};
use crate::engine::agent_port::AgentPort;
use crate::engine::error::Result;
use crate::engine::groups::rank::assert_user_role;
use crate::engine::machine::Engine;
use crate::limits::ROLE_TO_EDIT_GROUP;

/// `set`'s change to one feature, keeping what it does not name (a timer, a duration).
fn apply(slot: &mut Option<GroupFeature>, set: Option<GroupPref>) {
    let Some(set) = set else { return };
    let mut f = slot
        .take()
        .unwrap_or_else(|| GroupFeature::new(FeatureEnabled::Off));
    f.enable = if set.on {
        FeatureEnabled::On
    } else {
        FeatureEnabled::Off
    };
    f.role = set.role.map(|r| to_chat(r).as_str().to_owned());
    *slot = Some(f);
}

impl<A: AgentPort> Engine<A> {
    pub(crate) fn set_group_prefs(&mut self, group: i64, set: &GroupPrefs) -> Result<()> {
        let mut g = self.group(group)?;
        assert_user_role(&g, &ROLE_TO_EDIT_GROUP)?;
        let mut p = g.profile.group_preferences.clone().unwrap_or_default();
        let before = p.clone();
        apply(&mut p.direct_messages, set.direct_messages);
        apply(&mut p.full_delete, set.full_delete);
        apply(&mut p.reactions, set.reactions);
        apply(&mut p.voice, set.voice);
        apply(&mut p.files, set.files);
        apply(&mut p.simplex_links, set.simplex_links);
        apply(&mut p.reports, set.reports);
        apply(&mut p.history, set.history);
        if p == before {
            return Ok(());
        }
        g.profile.group_preferences = Some(p);
        let msg = ChatMessage::new(ChatEvent::GrpInfo(g.profile.clone()))?;
        self.save_group(g)?;
        self.send_to_group(group, &msg, None)?;
        self.state.chats_dirty = true;
        Ok(())
    }
}
