//! The chat list's changes: what is news, what the first list only fills, and members arriving.

use super::{Note, Roster, lift};
use crate::types::Event;
use crate::types::model::{
    ChatGroup, ChatMessage, ChatNotice, ChatRef, Delivery, Friend, FriendRequest, Membership,
    MessageAuthor, MessageBody,
};

fn friend(id: i64, ready: bool) -> Friend {
    Friend {
        id,
        name: format!("f{id}"),
        ready,
        ..Friend::default()
    }
}

fn group(id: i64, membership: Membership) -> ChatGroup {
    ChatGroup {
        id,
        name: format!("g{id}"),
        membership,
        ..ChatGroup::default()
    }
}

fn request(id: i64) -> FriendRequest {
    FriendRequest {
        id,
        name: format!("r{id}"),
    }
}

fn chats(friends: &[Friend], groups: &[ChatGroup], requests: &[FriendRequest]) -> Event {
    Event::Chats {
        friends: friends.to_vec(),
        groups: groups.to_vec(),
        requests: requests.to_vec(),
        chats: Vec::new(),
    }
}

#[test]
fn the_first_list_fills_the_cache_but_announces_only_what_waits_for_an_answer() {
    let mut r = Roster::default();
    let invited = group(2, Membership::Invited);
    let e = chats(
        &[friend(1, true)],
        &[group(1, Membership::Member), invited.clone()],
        &[request(7)],
    );
    let notes = lift(&e, &mut r);
    assert_eq!(
        notes,
        [Note::Request(request(7)), Note::GroupInvite(invited)]
    );
    assert_eq!((r.friends.len(), r.groups.len()), (1, 2));
}

#[test]
fn a_friend_is_added_when_ready_and_removed_when_gone() {
    let mut r = Roster::default();
    lift(&chats(&[], &[], &[]), &mut r);
    assert!(lift(&chats(&[friend(1, false)], &[], &[]), &mut r).is_empty());
    let notes = lift(&chats(&[friend(1, true)], &[], &[]), &mut r);
    assert_eq!(notes, [Note::FriendAdd(friend(1, true))]);
    assert!(lift(&chats(&[friend(1, true)], &[], &[]), &mut r).is_empty());
    let notes = lift(&chats(&[], &[], &[]), &mut r);
    assert_eq!(notes, [Note::FriendRemove(friend(1, true))]);
}

#[test]
fn a_request_is_announced_once() {
    let mut r = Roster::default();
    lift(&chats(&[], &[], &[request(1)]), &mut r);
    let notes = lift(&chats(&[], &[], &[request(1), request(2)]), &mut r);
    assert_eq!(notes, [Note::Request(request(2))]);
}

#[test]
fn a_group_is_added_on_becoming_a_member_and_removed_on_leaving() {
    let mut r = Roster::default();
    lift(&chats(&[], &[group(1, Membership::Invited)], &[]), &mut r);
    let joining = chats(&[], &[group(1, Membership::Joining)], &[]);
    assert!(lift(&joining, &mut r).is_empty());
    let notes = lift(&chats(&[], &[group(1, Membership::Member)], &[]), &mut r);
    assert_eq!(notes, [Note::GroupAdd(group(1, Membership::Member))]);
    let notes = lift(&chats(&[], &[group(1, Membership::Left)], &[]), &mut r);
    assert_eq!(notes, [Note::GroupRemove(group(1, Membership::Member))]);
    assert_eq!(
        r.groups[&1].membership,
        Membership::Left,
        "the cache keeps it"
    );
}

#[test]
fn a_joined_notice_in_a_group_also_reports_the_member() {
    let m = ChatMessage {
        id: 5,
        chat: ChatRef::Group(3),
        author: MessageAuthor::Member("bob".into()),
        body: MessageBody::Notice(ChatNotice::MemberJoined("bob".into())),
        at: 0,
        delivery: Delivery::Received,
        unread: true,
        reply: None,
        reactions: Vec::new(),
        edited: false,
        mentions: Vec::new(),
        timed: None,
    };
    let notes = lift(&Event::Messages(vec![m.clone()]), &mut Roster::default());
    let member = Note::MemberAdd {
        group: 3,
        name: "bob".into(),
    };
    assert_eq!(notes, [Note::Message(m), member]);
}
