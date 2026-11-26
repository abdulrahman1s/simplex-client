//! A post as the channel (`CDChannelRcv`): no member is named, it is kept once whichever relay brings it first, and its edits and deletion find it by its shared id (Subscriber.hs `newGroupContentMessage` with no member, `groupMessageUpdate`, `groupMessageDelete` with `asGroup`).

use crate::chat::container::MsgContainer;
use crate::chat::{ChatEvent, ChatMessage};
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::files::edit_caption;
use crate::engine::groups::policy::{Feature, allowed};
use crate::engine::machine::Engine;
use crate::engine::records::ItemRecord;
use crate::types::Event;
use crate::types::model::{ChatRef, MessageAuthor, MessageBody};

fn dropped(why: &str) -> crate::engine::error::EngineError {
    fail(Code::RequestFailed, why.to_owned())
}

impl<A: AgentPort> Engine<A> {
    /// `at`: the forward's time. `verified`: an owner's signed post.
    pub(crate) fn on_channel_post(
        &mut self,
        group: i64,
        msg: ChatMessage,
        at: i64,
        verified: bool,
    ) -> Result<()> {
        let shared = msg
            .msg_id
            .map(|m| m.0)
            .ok_or_else(|| dropped("a channel post without an id"))?;
        match msg.event {
            ChatEvent::MsgNew(c) => self.channel_post_new(group, shared, c, at, verified),
            ChatEvent::MsgUpdate {
                msg_id, content, ..
            } => {
                let chat = ChatRef::Group(group);
                let Some((id, item)) = self.item_by_shared(chat, None, &msg_id.0)? else {
                    return self.channel_post_new(
                        group,
                        msg_id.0,
                        MsgContainer::simple(content),
                        at,
                        verified,
                    );
                };
                if item.body == MessageBody::Deleted {
                    return Err(dropped("x.msg.update: invalid message update"));
                }
                let mut changed = item.clone();
                edit_caption(&mut changed.body, content.text().to_owned());
                if changed.body == item.body {
                    return Err(dropped("x.msg.update: not changed"));
                }
                changed.edited = true;
                self.save_changed(chat, id, &changed)
            }
            ChatEvent::MsgDel { msg_id, .. } => {
                let chat = ChatRef::Group(group);
                let Some((id, item)) = self.item_by_shared(chat, None, &msg_id.0)? else {
                    return Ok(());
                };
                if item.body == MessageBody::Deleted {
                    return Err(dropped("x.msg.del: already deleted"));
                }
                // As the channel: fully deleted when the channel allows it, else marked.
                let full = allowed(
                    &self.group(group)?,
                    Feature::FullDelete,
                    &crate::chat::group::GroupMemberRole::Owner,
                );
                self.finish_delete_in(chat, id, item, full)
            }
            ChatEvent::MsgFileDescr { .. }
            | ChatEvent::MsgDeleted
            | ChatEvent::MsgReact { .. }
            | ChatEvent::File(_)
            | ChatEvent::FileAcpt { .. }
            | ChatEvent::FileAcptInv { .. }
            | ChatEvent::FileCancel { .. }
            | ChatEvent::Info { .. }
            | ChatEvent::Contact { .. }
            | ChatEvent::Member { .. }
            | ChatEvent::DirectDel
            | ChatEvent::GrpInv(_)
            | ChatEvent::GrpAcpt { .. }
            | ChatEvent::GrpLinkInv(_)
            | ChatEvent::GrpLinkReject(_)
            | ChatEvent::GrpLinkMem { .. }
            | ChatEvent::GrpLinkAcpt { .. }
            | ChatEvent::GrpRelayInv(_)
            | ChatEvent::GrpRelayAcpt { .. }
            | ChatEvent::GrpRelayTest { .. }
            | ChatEvent::GrpRelayNew { .. }
            | ChatEvent::GrpRelayReject { .. }
            | ChatEvent::GrpRelayCap(_)
            | ChatEvent::GrpMemNew { .. }
            | ChatEvent::GrpMemIntro { .. }
            | ChatEvent::GrpMemInv { .. }
            | ChatEvent::GrpMemFwd { .. }
            | ChatEvent::GrpMemInfo { .. }
            | ChatEvent::GrpMemRole { .. }
            | ChatEvent::GrpMemRestrict { .. }
            | ChatEvent::GrpMemCon { .. }
            | ChatEvent::GrpMemConAll { .. }
            | ChatEvent::GrpMemDel { .. }
            | ChatEvent::GrpLeave
            | ChatEvent::GrpDel
            | ChatEvent::GrpInfo(_)
            | ChatEvent::GrpPrefs(_)
            | ChatEvent::GrpDirectInv { .. }
            | ChatEvent::GrpRoster { .. }
            | ChatEvent::GrpRosterAck { .. }
            | ChatEvent::GrpRosterRequest { .. }
            | ChatEvent::GrpMsgForward { .. }
            | ChatEvent::InfoProbe(_)
            | ChatEvent::InfoProbeCheck(_)
            | ChatEvent::InfoProbeOk(_)
            | ChatEvent::CallInv { .. }
            | ChatEvent::CallOffer { .. }
            | ChatEvent::CallAnswer { .. }
            | ChatEvent::CallExtra { .. }
            | ChatEvent::CallEnd { .. }
            | ChatEvent::Ok
            | ChatEvent::FileChunk { .. }
            | ChatEvent::Unknown { .. } => Err(dropped("not a channel post")),
        }
    }

    /// Each relay brings the same post: the first is kept, the rest are dropped quietly.
    fn channel_post_new(
        &mut self,
        group: i64,
        shared: Vec<u8>,
        c: MsgContainer,
        at: i64,
        verified: bool,
    ) -> Result<()> {
        let chat = ChatRef::Group(group);
        if self.item_by_shared(chat, None, &shared)?.is_some() {
            return Ok(());
        }
        let mut g = self.group(group)?;
        if let Some((tag, json)) = self.data_in(&c) {
            let first = g.channel.as_mut().is_some_and(|ch| ch.first_sight(&shared));
            if !first {
                return Ok(());
            }
            self.save_group(g)?;
            self.emit(Event::Data {
                chat,
                member: None,
                tag,
                json,
            });
            return Ok(());
        }
        let body = MessageBody::Text(c.content.text().to_owned());
        let mut item = ItemRecord::new(
            MessageAuthor::Channel(g.profile.display_name.clone()),
            body,
            at,
        );
        item.timed = self.incoming_timed(chat, c.ttl, true)?;
        item.link = crate::engine::link::of(&c.content);
        item.forwarded = c.forward == Some(true);
        item.shared_id = Some(shared);
        item.verified = verified;
        let added = self.add_item(chat, &item)?;
        let shown = self.shown_last(chat, &item, added);
        g.unread += 1;
        self.save_group(g)?;
        self.emit(Event::Messages(vec![shown]));
        Ok(())
    }
}
