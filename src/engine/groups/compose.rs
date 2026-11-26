//! Send in a group: one item, the message to every current member (kept for those still connecting), with its quote and mentions (Commands.hs `sendGroupContentMessages_`; Internal.hs `prepareGroupMsg`, `getCIMentions`).

use crate::types::Event;
use crate::types::model::{ChatRef, Mention, MessageAuthor, MessageBody, Quote, Timed};

use super::mention::{mentioned_names, quote_mentions};
use super::policy::at_least;
use crate::chat::container::{MsgContainer, MsgMention, MsgMentions, QuotedMsg};
use crate::chat::group::GroupMemberRole;
use crate::chat::ids::MemberId;
use crate::chat::{ChatEvent, ChatMessage};
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::link::Outgoing;
use crate::engine::machine::Engine;
use crate::engine::quote::outgoing;
use crate::engine::records::{GroupRecord, ItemRecord, keys};
use crate::engine::requests::sized;
use crate::limits::{MAX_SENT_MENTIONS, ROLE_TO_SEND};

impl<A: AgentPort> Engine<A> {
    /// Commands.hs `assertUserGroupRole`: the user is in the group with at least `min`; and,
    /// as `getGroupRecipients` asks before any send, joined, not still joining.
    pub(crate) fn assert_my_role(g: &GroupRecord, min: &GroupMemberRole) -> Result<()> {
        if !(g.status.current() && g.status.active()) {
            return Err(fail(
                Code::RequestFailed,
                "not an active member of the group",
            ));
        }
        if !at_least(&g.my_role, min) && g.channel.is_some() {
            // As 7.1.4 refuses it: `CEGroupUserRole` with the role needed.
            let why = format!("groupUserRole: required role {}", min.as_str());
            return Err(fail(Code::NotAllowed, why));
        }
        if !at_least(&g.my_role, min) {
            return Err(fail(Code::NotAllowed, "the user's group role is too low"));
        }
        Ok(())
    }

    /// The member id of member number `member`; 0 is the user.
    pub(crate) fn member_id_of(&self, g: &GroupRecord, member: i64) -> Result<MemberId> {
        if member == 0 {
            return Ok(g.my_member_id.clone());
        }
        Ok(self.member(g.id, member)?.member_id)
    }

    pub(crate) fn send_group_text(
        &mut self,
        group: i64,
        text: &str,
        reply_to: Option<i64>,
        mentions: &[(String, i64)],
        out: Outgoing<'_>,
    ) -> Result<()> {
        let g = self.group(group)?;
        Self::assert_my_role(&g, &ROLE_TO_SEND)?;
        let wire_text = quote_mentions(text, mentions.iter().map(|(n, _)| n.as_str()));
        let msg_mentions = self.outgoing_mentions(&g, &wire_text, mentions)?;
        let content = crate::engine::link::content_of(wire_text, out.link);
        let (mut container, reply) = match reply_to {
            Some(item) => {
                let (q, shown) = self.group_quote(&g, item)?;
                (MsgContainer::quote(q, content), Some(shown))
            }
            None => (MsgContainer::simple(content), None),
        };
        container.mentions = msg_mentions;
        let timed = self.outgoing_timed(ChatRef::Group(group))?;
        let timed = timed.map(|t| Timed {
            delete_at: t.delete_at.filter(|_| !out.live),
            ..t
        });
        container.ttl = timed.map(|t| t.ttl);
        container.forward = out.forward.then_some(true);
        container.live = out.live.then_some(true);
        let msg = ChatMessage::new(ChatEvent::MsgNew(container))?;
        sized(&msg)?;
        let sent = self.send_to_group(group, &msg, None)?;
        let now = self.clock.wall_ms();
        let mut item = ItemRecord::new(MessageAuthor::Me, MessageBody::Text(text.to_owned()), now);
        item.reply = reply;
        item.timed = timed;
        item.link = out.link.cloned();
        item.forwarded = out.forward;
        item.live = out.live;
        item.shared_id = msg.msg_id.map(|m| m.0);
        item.mentions = mentions
            .iter()
            .map(|(name, member)| Mention {
                name: name.clone(),
                member: *member,
                me: *member == 0,
            })
            .collect();
        let m = self.add_item(ChatRef::Group(group), &item)?;
        for (_, conn, agent_id) in sent {
            self.put_index(&keys::sent(&conn, agent_id), m.id)?;
        }
        self.emit(Event::Messages(vec![m]));
        Ok(())
    }

    /// `getCIMentions`: at most three, each named in the text, each a different member.
    fn outgoing_mentions(
        &self,
        g: &GroupRecord,
        wire_text: &str,
        mentions: &[(String, i64)],
    ) -> Result<MsgMentions> {
        if mentions.len() > MAX_SENT_MENTIONS {
            return Err(fail(Code::TooManyMentions, "more than 3 mentions"));
        }
        let named = mentioned_names(wire_text);
        let mut out = MsgMentions::new();
        let mut members = Vec::with_capacity(mentions.len());
        for (name, member) in mentions {
            if !named.contains(&name.as_str()) || members.contains(member) {
                return Err(fail(Code::RequestFailed, "invalid mention"));
            }
            members.push(*member);
            let member_id = self.member_id_of(g, *member)?;
            out.insert(name.clone(), MsgMention { member_id });
        }
        Ok(out)
    }

    /// `prepareGroupMsg` `quoteData`: a deleted item or a notice cannot be quoted; the quote
    /// names its author's member id (the user's own for the user's).
    pub(crate) fn group_quote(&mut self, g: &GroupRecord, item: i64) -> Result<(QuotedMsg, Quote)> {
        let q = self
            .item(ChatRef::Group(g.id), item)?
            .filter(|q| q.body != MessageBody::Deleted)
            .ok_or_else(|| fail(Code::RequestFailed, "no such item"))?;
        let (mut quoted, shown) =
            outgoing(item, &q).ok_or_else(|| fail(Code::RequestFailed, "no such item"))?;
        quoted.msg_ref.member_id = Some(self.member_id_of(g, q.member.unwrap_or(0))?);
        Ok((quoted, shown))
    }
}
