//! What the user does to conversations alone: delete here, clear, read up to, marks, aliases, message TTLs (Commands.hs `APIDeleteChatItem` `CIDMInternal`, `APIClearChat`, `APIChatItemsRead`, `APIChatUnread`, `APISetContactAlias`, `APISetChatItemTTL`, `APISetChatTTL`).

use crate::types::model::{ChatRef, MessageBody};
use crate::types::{Event, Request};

use super::timed::{later, timed_msg};
use super::{T, messages};
use crate::chat::ChatEvent;
use crate::chat::container::MsgContainer;
use crate::engine::agent_port::ConnId;

fn says(t: &mut T, conn: &ConnId, n: i64, text: &str) -> i64 {
    let out = t.peer_says(conn, n, ChatEvent::MsgNew(MsgContainer::text(text)));
    messages(&out)[0].id
}

fn unread(t: &T, friend: i64) -> u32 {
    t.e.state.friends[&friend].unread
}

#[test]
fn deleting_for_me_removes_it_here_and_tells_nobody() {
    let mut t = T::started();
    let (bob, conn) = t.friend("bob");
    let first = says(&mut t, &conn, 1, "one");
    says(&mut t, &conn, 2, "two");
    let sent = t.e.agent.sent().len();
    let out = t.req(Request::DeleteForMe {
        chat: ChatRef::Direct(bob),
        item: first,
    });
    assert_eq!(messages(&out)[0].body, MessageBody::Deleted);
    assert!(t.e.item(ChatRef::Direct(bob), first).unwrap().is_none());
    assert_eq!(t.e.agent.sent().len(), sent, "nobody is told");
    assert_eq!(unread(&t, bob), 1);
}

#[test]
fn clearing_a_chat_leaves_it_empty_read_and_without_timers() {
    let mut t = T::started();
    let (bob, conn) = t.friend("bob");
    says(&mut t, &conn, 1, "one");
    t.peer_says(&conn, 2, timed_msg("soon", Some(60)));
    let chat = ChatRef::Direct(bob);
    let out = t.req(Request::ClearChat(chat));
    assert!(out.contains(&Event::Cleared(chat)));
    assert!(t.e.items_page(chat, None, 10).unwrap().is_empty());
    assert_eq!(unread(&t, bob), 0);
    assert!(t.e.state.timed_unread.is_empty());
}

#[test]
fn reading_up_to_a_message_leaves_the_later_ones_unread() {
    let mut t = T::started();
    let (bob, conn) = t.friend("bob");
    says(&mut t, &conn, 1, "one");
    let second = says(&mut t, &conn, 2, "two");
    let third = says(&mut t, &conn, 3, "three");
    let chat = ChatRef::Direct(bob);
    t.req(Request::MarkReadUpTo { chat, item: second });
    assert_eq!(unread(&t, bob), 1);
    assert_eq!(t.e.state.friends[&bob].read_up_to, second);
    t.req(Request::MarkReadUpTo { chat, item: 1 });
    assert_eq!(
        t.e.state.friends[&bob].read_up_to, second,
        "a mark never moves back"
    );
    t.req(Request::MarkReadUpTo { chat, item: third });
    assert_eq!(unread(&t, bob), 0);
}

#[test]
fn marks_and_aliases_show_in_the_chat_list() {
    let mut t = T::started();
    let (bob, _) = t.friend("bob");
    let chat = ChatRef::Direct(bob);
    t.req(Request::SetUnreadMark { chat, on: true });
    t.req(Request::SetAlias {
        chat,
        alias: " Bobby ".into(),
    });
    let out = t.req(Request::Refresh);
    let Some(Event::Chats { friends, chats, .. }) =
        out.iter().find(|e| matches!(e, Event::Chats { .. }))
    else {
        panic!("{out:?}")
    };
    assert_eq!(friends[0].alias, "Bobby");
    assert!(chats.iter().any(|c| c.chat == chat && c.marked_unread));
    let t = t.restarted();
    assert_eq!(t.e.state.friends[&bob].local.alias, "Bobby");
}

#[test]
fn old_messages_go_after_the_ttl_unless_a_chat_keeps_them() {
    let mut t = T::started();
    let (bob, bob_conn) = t.friend("bob");
    let (carol, carol_conn) = t.friend("carol");
    t.req(Request::SetMessageTtl {
        chat: None,
        ttl: Some(3600),
    });
    t.req(Request::SetMessageTtl {
        chat: Some(ChatRef::Direct(carol)),
        ttl: Some(0),
    });
    let old = says(&mut t, &bob_conn, 1, "old");
    let kept = says(&mut t, &carol_conn, 1, "kept");
    later(&mut t, 1800);
    assert!(t.e.item(ChatRef::Direct(bob), old).unwrap().is_some());
    later(&mut t, 1801);
    assert!(
        t.e.item(ChatRef::Direct(bob), old).unwrap().is_none(),
        "older than an hour"
    );
    assert!(t.e.item(ChatRef::Direct(carol), kept).unwrap().is_some());
    let neg = t.req(Request::SetMessageTtl {
        chat: None,
        ttl: Some(-1),
    });
    assert!(neg.iter().any(|e| matches!(e, Event::Failed { .. })));
}

#[test]
fn a_groups_messages_of_the_last_twelve_hours_stay() {
    let mut t = T::started();
    let (g, m) = t.group("team", &["bob"], crate::chat::group::GroupMemberRole::Owner);
    t.req(Request::SetMessageTtl {
        chat: Some(ChatRef::Group(g)),
        ttl: Some(60),
    });
    let out = t.peer_says(&m[0].1, 1, ChatEvent::MsgNew(MsgContainer::text("fresh")));
    let fresh = messages(&out)[0].id;
    later(&mut t, 3600);
    assert!(t.e.item(ChatRef::Group(g), fresh).unwrap().is_some());
    later(&mut t, 12 * 3600);
    assert!(t.e.item(ChatRef::Group(g), fresh).unwrap().is_none());
}
