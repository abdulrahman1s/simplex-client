//! Moderate: a moderator deletes a member's message for everyone (Commands.hs `APIDeleteMemberChatItem`, `delGroupChatItemsForMembers`).

use crate::types::model::{ChatRef, MessageBody};

use crate::chat::group::GroupMemberRole;
use crate::chat::ids::SharedMsgId;
use crate::chat::{ChatEvent, ChatMessage};
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::groups::policy::{self, Feature};
use crate::engine::groups::rank::{assert_user_role, rank};
use crate::engine::machine::Engine;

impl<A: AgentPort> Engine<A> {
    /// Only a message of a member whose role is not above the user's (`assertDeletable`); it
    /// goes for good when the group allows full deletion to the user, else it stays marked.
    pub(crate) fn moderate(&mut self, group: i64, item: i64) -> Result<()> {
        let g = self.group(group)?;
        assert_user_role(&g, &GroupMemberRole::Moderator)?;
        let chat = ChatRef::Group(group);
        let it =
            (self.item(chat, item)?).ok_or_else(|| fail(Code::RequestFailed, "no such item"))?;
        let shared = (it.shared_id.clone())
            .ok_or_else(|| fail(Code::RequestFailed, "invalid chat item delete"))?;
        if it.body == MessageBody::Deleted {
            return Err(fail(Code::RequestFailed, "already deleted"));
        }
        let author = match it.member {
            None => g.my_member_id.clone(),
            Some(m) => {
                let m = self.member(group, m)?;
                if rank(&g.my_role) < rank(&m.role) {
                    return Err(fail(
                        Code::NotAllowed,
                        "the author's role is above the user's",
                    ));
                }
                m.member_id
            }
        };
        let msg = ChatMessage::new(ChatEvent::MsgDel {
            msg_id: SharedMsgId(shared),
            member_id: Some(author),
            scope: None,
            only_history: false,
        })?;
        self.send_to_group(group, &msg, None)?;
        let full = policy::allowed(&g, Feature::FullDelete, &g.my_role);
        self.finish_delete_in(chat, item, it, full)
    }
}
