//! Address switches and re-syncs told in the chat (Subscriber.hs `SWITCH`, `RSYNC` `ratchetSyncEventItem`; Commands.hs `APISyncContactRatchet`).

use crate::types::model::{
    ChatMessage, ChatNotice, ChatRef, Delivery, MessageAuthor, MessageBody, SwitchPhase, SyncStatus,
};

use super::{T, messages};
use crate::agent::client::SwitchPhase as AgentPhase;
use crate::agent::resync::SyncState;
use crate::chat::group::GroupMemberRole;
use crate::engine::agent_port::AgentEvent;

fn one(out: &[crate::types::Event]) -> ChatMessage {
    let [m] = messages(out).try_into().unwrap();
    m
}

fn listed(out: &[crate::types::Event]) -> bool {
    out.iter()
        .any(|e| matches!(e, crate::types::Event::Chats { .. }))
}

fn switch(phase: SwitchPhase) -> MessageBody {
    MessageBody::Notice(ChatNotice::AddressSwitch {
        phase,
        member: None,
    })
}

fn synced(status: SyncStatus, member: Option<&str>) -> MessageBody {
    MessageBody::Notice(ChatNotice::RatchetSync {
        status,
        member: member.map(str::to_owned),
    })
}

#[test]
fn a_friend_moving_the_address_we_send_to_is_told_started_then_changed_in_their_chat() {
    let mut t = T::started();
    let (bob, conn) = t.friend("bob");
    for (agent, phase) in [
        (AgentPhase::Started, SwitchPhase::Started),
        (AgentPhase::Completed, SwitchPhase::Completed),
    ] {
        let out = t.agent(AgentEvent::Switch { conn, phase: agent });
        assert!(listed(&out), "the chat list shows it");
        let m = one(&out);
        assert_eq!(m.body, switch(phase));
        assert_eq!(
            (m.chat, m.author),
            (ChatRef::Direct(bob), MessageAuthor::Friend)
        );
    }
}

#[test]
fn a_members_address_switch_is_told_in_the_group_as_theirs() {
    let mut t = T::started();
    let (group, members) = t.group("team", &["bob"], GroupMemberRole::Owner);
    let conn = members[0].1;
    let phase = AgentPhase::Completed;
    let m = one(&t.agent(AgentEvent::Switch { conn, phase }));
    assert_eq!(m.body, switch(SwitchPhase::Completed));
    assert_eq!(m.chat, ChatRef::Group(group));
    assert_eq!(m.author, MessageAuthor::Member("bob".into()));
}

#[test]
fn a_resync_back_to_ok_is_told_as_synchronized() {
    let mut t = T::started();
    let (_, conn) = t.friend("bob");
    let state = SyncState::Ok;
    let m = one(&t.agent(AgentEvent::RatchetSync { conn, state }));
    assert_eq!(m.body, synced(SyncStatus::Ok, None));
    assert_eq!(m.author, MessageAuthor::Friend);
}

#[test]
fn allowed_and_started_states_are_not_told_as_sync_items() {
    let mut t = T::started();
    let (_, conn) = t.friend("bob");
    for state in [SyncState::Allowed, SyncState::Started] {
        let out = t.agent(AgentEvent::RatchetSync { conn, state });
        assert!(messages(&out).is_empty(), "{state:?}");
    }
}

#[test]
fn a_required_resync_the_engine_starts_is_told_as_the_users_own() {
    let mut t = T::started();
    let (bob, conn) = t.friend("bob");
    let state = SyncState::Required;
    let m = one(&t.agent(AgentEvent::RatchetSync { conn, state }));
    assert_eq!(m.body, synced(SyncStatus::Started, None));
    assert_eq!(
        (m.chat, m.author),
        (ChatRef::Direct(bob), MessageAuthor::Me)
    );
    assert_eq!(m.delivery, Delivery::Sent, "never sent, never pending");
}

#[test]
fn a_resync_started_with_a_member_is_the_users_item_naming_the_member() {
    let mut t = T::started();
    let (group, members) = t.group("team", &["bob"], GroupMemberRole::Owner);
    let conn = members[0].1;
    let state = SyncState::Required;
    let m = one(&t.agent(AgentEvent::RatchetSync { conn, state }));
    assert_eq!(m.body, synced(SyncStatus::Started, Some("bob")));
    assert_eq!(
        (m.chat, m.author),
        (ChatRef::Group(group), MessageAuthor::Me)
    );
}
