//! With `roster_deltas` the list goes out as what changed: whole at the start and on a refresh, and rebuilt from the changes it equals a whole list.

use crate::types::model::ChatRef;
use crate::types::{Event, Request};

use super::{T, messages};
use crate::chat::ChatEvent;
use crate::chat::container::MsgContainer;
use crate::chat::group::GroupMemberRole;
use crate::roster::{Roster, lift};

/// Started with deltas, recording all it puts out.
fn deltas() -> T {
    let mut t = T::new();
    let mut hello = t.dir.hello("alice");
    hello.roster_deltas = true;
    t.listed.record = Some(Vec::new());
    t.e.start(&hello).unwrap();
    t.out();
    t
}

fn whole(out: &[Event]) -> usize {
    out.iter()
        .filter(|e| matches!(e, Event::Chats { .. }))
        .count()
}

fn changes(out: &[Event]) -> Vec<&Event> {
    out.iter()
        .filter(|e| matches!(e, Event::ChatsChanged { .. }))
        .collect()
}

#[test]
fn with_deltas_a_changed_friend_goes_out_alone_and_a_removed_one_by_id() {
    let mut t = deltas();
    let (bob, _) = t.friend("bob");
    let (carol, _) = t.friend("carol");
    let chat = ChatRef::Direct(bob);
    let out = t.req(Request::SetMuted { chat, on: true });
    assert_eq!(whole(&out), 0);
    let [
        Event::ChatsChanged {
            friends,
            groups,
            removed_friends,
            requests,
            ..
        },
    ] = changes(&out)[..]
    else {
        panic!("{out:?}")
    };
    assert_eq!(friends.iter().map(|f| f.id).collect::<Vec<_>>(), [bob]);
    assert!(friends[0].muted);
    assert!(groups.is_empty() && removed_friends.is_empty() && requests.is_none());
    let out = t.req(Request::RemoveFriend(carol));
    let [
        Event::ChatsChanged {
            removed_friends, ..
        },
    ] = changes(&out)[..]
    else {
        panic!("{out:?}")
    };
    assert_eq!(removed_friends, &[carol]);
}

#[test]
fn with_deltas_a_refresh_sends_the_whole_list() {
    let mut t = deltas();
    t.friend("bob");
    let out = t.req(Request::Refresh);
    assert_eq!((whole(&out), changes(&out).len()), (1, 0));
}

#[test]
fn without_deltas_no_change_goes_out_alone() {
    let mut t = T::started();
    let (bob, _) = t.friend("bob");
    let out = t.req(Request::SetMuted {
        chat: ChatRef::Direct(bob),
        on: true,
    });
    assert_eq!((whole(&out), changes(&out).len()), (1, 0));
}

#[test]
fn the_roster_rebuilt_from_the_changes_equals_a_whole_list() {
    let mut t = deltas();
    let (bob, bob_conn) = t.friend("bob");
    let (g, _) = t.group("team", &["carol", "dan"], GroupMemberRole::Owner);
    t.req(Request::SetMuted {
        chat: ChatRef::Group(g),
        on: true,
    });
    t.req(Request::SetAlias {
        chat: ChatRef::Direct(bob),
        alias: "B".into(),
    });
    let said = t.peer_says(&bob_conn, 1, ChatEvent::MsgNew(MsgContainer::text("hi")));
    assert_eq!(messages(&said).len(), 1);
    let (dave, _) = t.friend("dave");
    t.req(Request::RemoveFriend(dave));
    let heard = t.listed.record.take().unwrap();
    assert_eq!(whole(&heard), 1, "the start's only");
    assert!(changes(&heard).len() > 3);
    let mut roster = Roster::default();
    for e in &heard {
        lift(e, &mut roster);
    }
    let Event::Chats {
        friends, groups, ..
    } = t.e.chats()
    else {
        panic!()
    };
    let by_id = |f: &crate::types::model::Friend| (f.id, f.clone());
    assert_eq!(roster.friends, friends.iter().map(by_id).collect());
    assert_eq!(
        roster.groups,
        groups.iter().map(|g| (g.id, g.clone())).collect()
    );
    assert!(roster.friends[&bob].alias == "B" && roster.groups[&g].muted);
}
