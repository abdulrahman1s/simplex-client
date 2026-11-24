//! Address switches and ratchet re-syncs told in the friend's or member's chat (Subscriber.hs `SWITCH`, `ratchetSyncEventItem`; Commands.hs `APISyncContactRatchet`, `APISyncGroupMemberRatchet`).

use crate::agent::client::SwitchPhase as AgentPhase;
use crate::types::Event;
use crate::types::model::{ChatNotice, ChatRef, Delivery, MessageAuthor, MessageBody, SwitchPhase};

use crate::engine::agent_port::{AgentPort, ConnId};
use crate::engine::error::Result;
use crate::engine::machine::Engine;
use crate::engine::records::ItemRecord;

/// Whose doing a connection event is: chat's `CIRcvConnEvent` or `CISndConnEvent`.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum By {
    Peer,
    User,
}

impl<A: AgentPort> Engine<A> {
    /// `SWITCH QDSnd`: the peer moves the queue we send to. The agent reports only the two
    /// phases chat shows (`SPStarted`, `SPCompleted`).
    pub(crate) fn on_switch(&mut self, conn: &ConnId, phase: AgentPhase) -> Result<()> {
        let phase = match phase {
            AgentPhase::Started => SwitchPhase::Started,
            AgentPhase::Completed => SwitchPhase::Completed,
        };
        self.conn_event(conn, By::Peer, |member| ChatNotice::AddressSwitch {
            phase,
            member,
        })
    }

    /// The notice in the chat `conn` belongs to; nothing for a connection of neither. The user's
    /// own event in a group names the member (`groupMemberRef`).
    pub(super) fn conn_event(
        &mut self,
        conn: &ConnId,
        by: By,
        notice: impl FnOnce(Option<String>) -> ChatNotice,
    ) -> Result<()> {
        if let Some(friend) = self.state.friend_of(conn) {
            return match by {
                By::Peer => {
                    self.notice(friend, notice(None))?;
                    self.state.chats_dirty = true;
                    Ok(())
                }
                By::User => self.own_event(ChatRef::Direct(friend), notice(None)),
            };
        }
        let Some((group, member)) = self.state.member_of(conn) else {
            return Ok(());
        };
        match by {
            By::Peer => self.group_notice(group, Some(member), notice(None)),
            By::User => {
                let name = self.state.members.get(&(group, member));
                let name = name.map(|m| m.profile.display_name.clone());
                self.own_event(ChatRef::Group(group), notice(name))
            }
        }
    }

    /// An item of the user's that is never sent, so it is not left showing "sending".
    fn own_event(&mut self, chat: ChatRef, notice: ChatNotice) -> Result<()> {
        let body = MessageBody::Notice(notice);
        let mut item = ItemRecord::new(MessageAuthor::Me, body, self.clock.wall_ms());
        item.delivery = Delivery::Sent;
        let m = self.add_item(chat, &item)?;
        self.emit(Event::Messages(vec![m]));
        self.state.chats_dirty = true;
        Ok(())
    }
}
