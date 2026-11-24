//! A friend's reaction: one of the set, on an item with content, within their three (Subscriber.hs `directMsgReaction`, `reactionAllowed`).

use crate::types::Event;
use crate::types::model::{ChatRef, Reaction};

use crate::chat::reaction::MsgReaction;
use crate::engine::agent_port::AgentPort;
use crate::engine::error::Result;
use crate::engine::machine::Engine;
use crate::engine::records::friend::Feature;
use crate::engine::requests::has_content;
use crate::limits::MAX_REACTIONS;

/// The page's form of an emoji: ❤ travels without its variation selector.
fn display(c: char) -> String {
    if c == '\u{2764}' {
        "\u{2764}\u{fe0f}".to_owned()
    } else {
        c.to_string()
    }
}

/// The friend's part of a count: whatever is not the user's.
fn theirs(r: &Reaction) -> bool {
    r.count > u32::from(r.mine)
}

/// The friend's reaction added or taken away; the user's own part of the count stays.
fn react(reactions: &mut Vec<Reaction>, emoji: String, add: bool) {
    let i = reactions.iter().position(|r| r.emoji == emoji);
    let had = i.is_some_and(|i| theirs(&reactions[i]));
    match (i, add) {
        (Some(i), true) if !had => reactions[i].count += 1,
        (None, true) => reactions.push(Reaction {
            emoji,
            count: 1,
            mine: false,
            who: Vec::new(),
        }),
        (Some(i), false) if had => {
            reactions[i].count -= 1;
            if reactions[i].count == 0 {
                reactions.remove(i);
            }
        }
        (Some(_) | None, true | false) => {}
    }
}

impl<A: AgentPort> Engine<A> {
    /// One outside the set, on a notice or a deleted item, or a fourth one is ignored.
    pub(crate) fn on_msg_react(
        &mut self,
        friend: i64,
        shared: &[u8],
        reaction: &MsgReaction,
        add: bool,
    ) -> Result<()> {
        let emoji = match reaction {
            MsgReaction::Emoji(c) => display(*c),
            MsgReaction::Unknown(_) => return Ok(()),
        };
        if !self.friend(friend)?.enabled(Feature::Reactions).for_contact {
            return Ok(());
        }
        let Some((id, mut item)) = self.item_by_shared(ChatRef::Direct(friend), None, shared)?
        else {
            return Ok(());
        };
        let given = item.reactions.iter().filter(|r| theirs(r)).count();
        if !has_content(&item.body) || (add && given >= MAX_REACTIONS) {
            return Ok(());
        }
        react(&mut item.reactions, emoji, add);
        let m = self.change_item(ChatRef::Direct(friend), id, &item)?;
        self.emit(Event::Updated(vec![m]));
        Ok(())
    }
}
