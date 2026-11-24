//! A message that did not decrypt: a notice, or one more on the notice just before it (Subscriber.hs `RSYNC` `processErr`, `mdeUpdatedCI`).

use crate::types::Event;
use crate::types::model::{ChatNotice, ChatRef, DecryptError, MessageBody};

use crate::engine::agent_port::{AgentPort, ConnId};
use crate::engine::error::Result;
use crate::engine::machine::Engine;
use crate::engine::records::{ItemRecord, decode, keys};

/// The count the notice shows after `n` more: skipped messages are counted afresh each time.
fn merged(error: DecryptError, count: u32, n: u32) -> u32 {
    match error {
        DecryptError::TooManySkipped => n,
        DecryptError::Header | DecryptError::Earlier | DecryptError::Other => {
            count.saturating_add(n)
        }
    }
}

impl<A: AgentPort> Engine<A> {
    pub(crate) fn on_decrypt_error(
        &mut self,
        conn: &ConnId,
        error: DecryptError,
        n: u32,
    ) -> Result<()> {
        let (chat, member) = if let Some(friend) = self.state.friend_of(conn) {
            if self.friend(friend)?.blocked {
                return Ok(());
            }
            (ChatRef::Direct(friend), None)
        } else if let Some((group, member)) = self.state.member_of(conn) {
            (ChatRef::Group(group), Some(member))
        } else {
            return Ok(());
        };
        if self.add_to_last(chat, member, error, n)? {
            return Ok(());
        }
        let notice = ChatNotice::DecryptionError { error, count: n };
        match (chat, member) {
            (ChatRef::Group(group), member) => self.group_notice(group, member, notice),
            (ChatRef::Direct(friend), _) => self.notice(friend, notice),
        }
    }

    /// The chat's newest item is the same notice from the same peer: it counts these too.
    fn add_to_last(
        &mut self,
        chat: ChatRef,
        member: Option<i64>,
        error: DecryptError,
        n: u32,
    ) -> Result<bool> {
        let Some((id, bytes)) = self.store.page(&keys::chat(chat), None, 1)?.pop() else {
            return Ok(false);
        };
        let mut item: ItemRecord = decode(&bytes)?;
        let MessageBody::Notice(ChatNotice::DecryptionError { error: last, count }) = item.body
        else {
            return Ok(false);
        };
        if last != error || item.member != member {
            return Ok(false);
        }
        item.body = MessageBody::Notice(ChatNotice::DecryptionError {
            error,
            count: merged(error, count, n),
        });
        let m = self.change_item(chat, id, &item)?;
        self.emit(Event::Updated(vec![m]));
        Ok(true)
    }
}
