//! ContactMember: the user asks a group member to talk directly, with a one-time link sent over the group connection (Commands.hs `APICreateMemberContact`, `APISendMemberContactInvitation`).

use serde::{Deserialize, Serialize};

use crate::types::Event;
use crate::types::model::{ChatRef, MessageAuthor, MessageBody};

use super::role::{at_least, direct_messages_allowed};
use crate::chat::content::MsgContent;
use crate::chat::group::GroupMemberRole;
use crate::chat::{ChatEvent, ChatMessage};
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::machine::Engine;
use crate::engine::records::{ConnKind, ConnRecord, FriendRecord, ItemRecord, NS, decode, keys};

/// What waits for the invitation's link: the member it goes to and what goes with it.
#[derive(Serialize, Deserialize)]
struct Waiting {
    group: i64,
    member: i64,
    text: Option<String>,
}

/// The member contact waiting for its link. Eviction: when the link goes out, or with the friend.
fn waiting_key(friend: i64) -> Vec<u8> {
    format!("minv/{friend}").into_bytes()
}

impl<A: AgentPort> Engine<A> {
    /// The friend is made now, connecting; the member joins the link as with any invitation.
    pub(crate) fn contact_member(
        &mut self,
        group: i64,
        member: i64,
        text: Option<String>,
    ) -> Result<()> {
        let g = self.group(group)?;
        let mut m = self.member(group, member)?;
        if !g.status.current() || !at_least(&g.my_role, &GroupMemberRole::Author) {
            return Err(fail(Code::NotAllowed, "the user can't send in the group"));
        }
        if !direct_messages_allowed(&g, &g.my_role) {
            return Err(fail(Code::NotAllowed, "direct messages not allowed"));
        }
        if m.conn.is_none() || !m.status.current() {
            return Err(fail(Code::RequestFailed, "the member is not connected"));
        }
        // A friend removed since leaves the member free to be asked again.
        if m.friend
            .is_some_and(|f| self.state.friends.contains_key(&f))
        {
            return Err(fail(Code::RequestFailed, "member contact already exists"));
        }
        // `IKPQOff`, as upstream's member contacts start.
        let conn = self
            .agent
            .create_invitation(&mut self.store, false, false, Vec::new())?;
        let id = self.store.next_id(keys::FRIEND_SEQ)?;
        let mut f = FriendRecord::new(id, conn, self.clock.wall_ms());
        f.name.clone_from(&m.profile.display_name);
        f.prefs.clone_from(&m.profile.preferences);
        f.pq = false;
        self.save_friend(f)?;
        self.save_conn(ConnRecord {
            conn,
            kind: ConnKind::Friend { id },
        })?;
        m.friend = Some(id);
        self.save_member(m)?;
        self.put(
            &waiting_key(id),
            &Waiting {
                group,
                member,
                text,
            },
        )?;
        self.state.chats_dirty = true;
        Ok(())
    }

    /// The link of a member contact's invitation arrived: it goes to the member. `false` when
    /// the friend has no member waiting.
    pub(crate) fn send_member_contact(&mut self, friend: i64, link: String) -> Result<bool> {
        let Some(bytes) = self.store.get(NS, &waiting_key(friend))? else {
            return Ok(false);
        };
        let w: Waiting = decode(&bytes)?;
        self.store.delete(NS, &waiting_key(friend))?;
        let content = w.text.clone().map(|text| MsgContent::Text { text });
        let msg = ChatMessage::new(ChatEvent::GrpDirectInv {
            conn_req: link,
            content,
            scope: None,
        })?;
        self.send_to_members(w.group, &[w.member], &msg)?;
        if let Some(text) = w.text {
            let item = ItemRecord::new(
                MessageAuthor::Me,
                MessageBody::Text(text),
                self.clock.wall_ms(),
            );
            let m = self.add_item(ChatRef::Direct(friend), &item)?;
            self.emit(Event::Messages(vec![m]));
        }
        Ok(true)
    }
}
