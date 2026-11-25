//! DeleteGroup: the group goes from here with everything kept about it; an owner deletes it for every member first (Commands.hs `APIDeleteChat` `CTGroup`).

use crate::types::model::ChatRef;

use crate::chat::group::GroupMemberRole;
use crate::chat::{ChatEvent, ChatMessage};
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::files::transfers::owner;
use crate::engine::machine::Engine;
use crate::engine::records::{NS, keys};
use crate::store::Op;

/// Items deleted per batch.
const PAGE: u32 = 500;

impl<A: AgentPort> Engine<A> {
    /// Only an owner may delete a group the user is still in (`canDelete`).
    pub(crate) fn delete_group(&mut self, group: i64) -> Result<()> {
        let g = self.group(group)?;
        let owner_role = g.my_role == GroupMemberRole::Owner;
        if !owner_role && g.status.current() {
            return Err(fail(Code::NotAllowed, "only an owner deletes the group"));
        }
        // Commands.hs `doSendDel`: the connections wait for their sends only if it went.
        let told = owner_role && g.status.active();
        if told {
            self.send_to_group(group, &ChatMessage::new(ChatEvent::GrpDel)?, None)?;
        }
        let chat = ChatRef::Group(group);
        self.forget_files(owner(chat))?;
        self.delete_group_link_if_exists(group)?;
        self.drop_group_conns(group, told)?;
        self.forget_group_items(chat)?;
        self.forget_timed(chat)?;
        let mut prefixes = vec![
            keys::shared_prefix(chat),
            keys::group_reaction_prefix(group),
            keys::file_members_prefix(group),
            keys::file_prefix(owner(chat)),
            keys::member_prefix(group),
        ];
        prefixes.extend([
            format!("pend/{group}/").into_bytes(),
            format!("mblock/{group}/").into_bytes(),
        ]);
        let mut ops = vec![Op::Delete {
            ns: NS,
            key: keys::group(group),
        }];
        for prefix in prefixes {
            for key in self.store.keys_with_prefix(NS, &prefix)? {
                ops.push(Op::Delete { ns: NS, key });
            }
        }
        self.store.apply(ops)?;
        self.state.groups.remove(&group);
        self.state.members.retain(|(g, _), _| *g != group);
        self.state.last.remove(&chat);
        self.state.chats_dirty = true;
        Ok(())
    }

    fn forget_group_items(&mut self, chat: ChatRef) -> Result<()> {
        let items = keys::chat(chat);
        loop {
            let page = self.store.page(&items, None, PAGE)?;
            if page.is_empty() {
                return Ok(());
            }
            let ops = page
                .into_iter()
                .map(|(id, _)| Op::DeleteItem {
                    chat_ref: items.clone(),
                    id,
                })
                .collect();
            self.store.apply(ops)?;
        }
    }
}
