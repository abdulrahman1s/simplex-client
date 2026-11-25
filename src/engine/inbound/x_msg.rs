//! A friend's messages: `x.msg.new` (with its quote), and their edits and deletions.

use crate::types::Event;
use crate::types::model::{ChatFeature, ChatRef, MessageAuthor, MessageBody, Quote};

use crate::chat::container::{MsgContainer, QuotedMsg};
use crate::chat::content::MsgContent;
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::machine::Engine;
use crate::engine::quote::shown_quote;
use crate::engine::records::ItemRecord;
use crate::limits::RCV_DELETE_WINDOW_MS;

impl<A: AgentPort> Engine<A> {
    /// A blocked friend's message is deleted on arrival: never kept, never shown.
    pub(crate) fn on_msg_new(
        &mut self,
        friend: i64,
        shared: Option<Vec<u8>>,
        c: MsgContainer,
    ) -> Result<()> {
        let mut f = self.friend(friend)?;
        if f.blocked {
            return Ok(());
        }
        if let Some((tag, json)) = self.data_in(&c) {
            let chat = ChatRef::Direct(friend);
            self.emit(Event::Data {
                chat,
                member: None,
                tag,
                json,
            });
            return Ok(());
        }
        // A message not acked before a restart comes again; it is kept once.
        if let Some(s) = &shared
            && self
                .item_by_shared(ChatRef::Direct(friend), None, s)?
                .is_some()
        {
            return Ok(());
        }
        // Subscriber.hs `newContentMessage`: a voice message the user does not allow is not kept.
        if matches!(c.content, MsgContent::Voice { .. })
            && !self.enabled(&f, ChatFeature::Voice).for_contact
        {
            return Err(fail(Code::RequestFailed, "voice messages are not allowed"));
        }
        let size = c.file.as_ref().map_or(0, |i| i.file_size);
        let offer = c
            .file
            .as_ref()
            .map(|inv| crate::engine::files::offer(&c.content, inv))
            .transpose()?;
        let body = offer.as_ref().map_or_else(
            || MessageBody::Text(c.content.text().to_owned()),
            |(b, _)| b.clone(),
        );
        let mut item = ItemRecord::new(MessageAuthor::Friend, body, self.clock.wall_ms());
        item.timed = self.incoming_timed(ChatRef::Direct(friend), c.ttl, false)?;
        item.link = crate::engine::link::of(&c.content);
        item.forwarded = c.forward == Some(true);
        item.live = c.live == Some(true);
        item.shared_id = shared;
        item.reply = c
            .quote
            .map(|q| self.incoming_quote(friend, &q))
            .transpose()?;
        let m = self.add_item(ChatRef::Direct(friend), &item)?;
        f.unread += 1;
        self.save_friend(f)?;
        let id = m.id;
        self.emit(Event::Messages(vec![m]));
        if let Some((_, rec)) = offer {
            self.on_offered(friend, id, rec, size)?;
        }
        Ok(())
    }

    /// `sent` is from the sender's side: true means the friend wrote the quoted message.
    fn incoming_quote(&mut self, friend: i64, q: &QuotedMsg) -> Result<Quote> {
        let id = match &q.msg_ref.msg_id {
            Some(shared) => self
                .item_by_shared(ChatRef::Direct(friend), None, &shared.0)?
                .map(|(id, _)| id),
            None => None,
        };
        let author = if q.msg_ref.sent {
            MessageAuthor::Friend
        } else {
            MessageAuthor::Me
        };
        Ok(shown_quote(id, author, q.content.text()))
    }

    /// Their own message, changed by `f`; anything else is left alone.
    fn change_theirs(
        &mut self,
        friend: i64,
        shared: &[u8],
        f: impl FnOnce(&mut ItemRecord),
    ) -> Result<()> {
        let Some((id, mut item)) = self.item_by_shared(ChatRef::Direct(friend), None, shared)?
        else {
            return Ok(());
        };
        if item.author != MessageAuthor::Friend || item.body == MessageBody::Deleted {
            return Ok(());
        }
        f(&mut item);
        let m = self.change_item(ChatRef::Direct(friend), id, &item)?;
        self.emit(Event::Updated(vec![m]));
        Ok(())
    }

    pub(crate) fn on_msg_update(
        &mut self,
        friend: i64,
        shared: &[u8],
        content: &MsgContent,
        live: bool,
    ) -> Result<()> {
        let text = content.text().to_owned();
        let link = crate::engine::link::of(content);
        // A live message's updates are its writing, not edits (`edited = itemLive /= Just True`).
        self.change_theirs(friend, shared, |i| {
            crate::engine::files::edit_caption(&mut i.body, text);
            i.link = link;
            i.edited |= !i.live;
            i.live = i.live && live;
        })
    }

    /// Subscriber.hs `messageDelete`: gone for good when the preferences allow their full
    /// delete; only if the relay took the deletion within `rcvItemDeletable`'s 78 hours of
    /// the item, so one that waited while the user was offline still counts.
    pub(crate) fn on_msg_del(&mut self, friend: i64, shared: &[u8], broker_ms: i64) -> Result<()> {
        let Some((id, item)) = self.item_by_shared(ChatRef::Direct(friend), None, shared)? else {
            return Ok(());
        };
        if item.author != MessageAuthor::Friend || item.body == MessageBody::Deleted {
            return Ok(());
        }
        if broker_ms.saturating_sub(item.at) >= RCV_DELETE_WINDOW_MS {
            return Err(fail(
                Code::RequestFailed,
                "x.msg.del: contact attempted invalid message delete",
            ));
        }
        let full = self
            .enabled(&self.friend(friend)?, ChatFeature::FullDelete)
            .for_contact;
        self.finish_delete(friend, id, item, full)
    }
}
