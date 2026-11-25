//! The conversation says when the timer that applies to it changed (Internal.hs `createRcvFeatureItems`, `createSndFeatureItems`).

use crate::types::Event;
use crate::types::model::{ChatNotice, ChatRef, MessageAuthor, MessageBody};

use super::rules::friend_ttl;
use crate::engine::agent_port::AgentPort;
use crate::engine::error::Result;
use crate::engine::machine::Engine;
use crate::engine::records::ItemRecord;

impl<A: AgentPort> Engine<A> {
    /// When the timer that applies changed from `before`, the conversation says so, as
    /// written by `author`.
    pub(crate) fn timer_notice(
        &mut self,
        friend: i64,
        before: Option<Option<i64>>,
        author: MessageAuthor,
    ) -> Result<()> {
        let now = friend_ttl(self.state.user.prefs.as_ref(), &self.friend(friend)?);
        if now.flatten() == before.flatten() {
            return Ok(());
        }
        let notice = MessageBody::Notice(ChatNotice::Timer(now.flatten()));
        let item = ItemRecord::new(author, notice, self.clock.wall_ms());
        let m = self.add_item(ChatRef::Direct(friend), &item)?;
        self.emit(Event::Messages(vec![m]));
        Ok(())
    }

    /// An owner changed the group's timer (`x.grp.info` or `x.grp.prefs`).
    pub(crate) fn group_timer_notice(
        &mut self,
        group: i64,
        author: i64,
        before: Option<Option<i64>>,
        now: Option<Option<i64>>,
    ) -> Result<()> {
        if now.flatten() == before.flatten() {
            return Ok(());
        }
        self.group_notice(group, Some(author), ChatNotice::Timer(now.flatten()))
    }
}
