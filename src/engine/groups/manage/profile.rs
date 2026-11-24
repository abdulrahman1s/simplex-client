//! SetGroupProfile: an owner renames the group or changes its description, sent in `x.grp.info` (Commands.hs `APIUpdateGroupProfile`, `runUpdateGroupProfile`).

use crate::types::model::ChatNotice;

use crate::chat::{ChatEvent, ChatMessage};
use crate::engine::agent_port::AgentPort;
use crate::engine::error::Result;
use crate::engine::groups::rank::assert_user_role;
use crate::engine::machine::Engine;
use crate::limits::ROLE_TO_EDIT_GROUP;

impl<A: AgentPort> Engine<A> {
    /// The name was checked with the limits; an empty description is none.
    pub(crate) fn set_group_profile(
        &mut self,
        group: i64,
        name: &str,
        description: Option<&str>,
    ) -> Result<()> {
        let mut g = self.group(group)?;
        assert_user_role(&g, &ROLE_TO_EDIT_GROUP)?;
        let name = name.trim().to_owned();
        let description = description.map(str::trim).filter(|d| !d.is_empty());
        let renamed = g.profile.display_name != name;
        if !renamed && g.profile.description.as_deref() == description {
            return Ok(());
        }
        g.profile.display_name.clone_from(&name);
        g.profile.description = description.map(str::to_owned);
        let msg = ChatMessage::new(ChatEvent::GrpInfo(g.profile.clone()))?;
        self.save_group(g)?;
        self.send_to_group(group, &msg, None)?;
        self.state.chats_dirty = true;
        if renamed {
            self.group_notice(group, None, ChatNotice::GroupNameChanged(name))?;
        }
        Ok(())
    }
}
