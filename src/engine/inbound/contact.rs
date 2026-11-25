//! A friend's own changes: a new profile (`x.info`), the contact deleted (`x.direct.del`).

use crate::types::Event;
use crate::types::model::{ChatNotice, ChatRef, MessageAuthor, MessageBody};

use crate::chat::profile::Profile;
use crate::engine::agent_port::AgentPort;
use crate::engine::error::Result;
use crate::engine::machine::Engine;
use crate::engine::records::ItemRecord;
use crate::engine::timed::adopt::apply_profile;
use crate::engine::timed::rules::friend_ttl;

impl<A: AgentPort> Engine<A> {
    pub(crate) fn notice(&mut self, friend: i64, notice: ChatNotice) -> Result<()> {
        let item = ItemRecord::new(
            MessageAuthor::Friend,
            MessageBody::Notice(notice),
            self.clock.wall_ms(),
        );
        let m = self.add_item(ChatRef::Direct(friend), &item)?;
        self.emit(Event::Messages(vec![m]));
        Ok(())
    }

    /// Only a new name is told in the conversation (codec `renamed`).
    pub(crate) fn on_x_info(&mut self, friend: i64, profile: &Profile) -> Result<()> {
        let mut f = self.friend(friend)?;
        let from = f.name.clone();
        let timer = friend_ttl(self.state.user.prefs.as_ref(), &f);
        apply_profile(self.state.user.prefs.as_ref(), &mut f, profile);
        let to = f.name.clone();
        self.save_friend(f)?;
        if from != to && !from.is_empty() {
            self.notice(friend, ChatNotice::NameChanged { from, to })?;
        }
        self.timer_notice(friend, timer, MessageAuthor::Friend)?;
        self.state.chats_dirty = true;
        Ok(())
    }

    /// The conversation stays and says so; the connection goes once this message is acked.
    pub(crate) fn on_direct_del(&mut self, friend: i64) -> Result<()> {
        let mut f = self.friend(friend)?;
        if f.gone {
            return Ok(());
        }
        f.gone = true;
        self.save_friend(f)?;
        self.notice(friend, ChatNotice::FriendDeleted)?;
        self.state.chats_dirty = true;
        Ok(())
    }
}
