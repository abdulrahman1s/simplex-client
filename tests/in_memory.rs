//! Two clients on the in-memory SMP server (feature `testing`): they connect, talk, send application data and hear roster deltas, with no network.
#![cfg(feature = "testing")]
#![allow(clippy::unwrap_used, reason = "tests")]

use std::path::PathBuf;
use std::time::Duration;

use simplex_client::agent::testing::{Dir, FakeSmp};
use simplex_client::{Client, Event};

fn client(dir: &Dir, name: &str, server: &std::sync::Arc<FakeSmp>) -> Client {
    client_with(dir, name, server, false)
}

fn client_with(dir: &Dir, name: &str, server: &std::sync::Arc<FakeSmp>, deltas: bool) -> Client {
    let folder: PathBuf = dir.0.join(name);
    Client::builder(folder)
        .name(name)
        .roster_deltas(deltas)
        .key("test key")
        .data_tag("app.ping")
        .log(std::io::sink())
        .timeout(Duration::from_secs(10))
        .start_in_memory(server)
        .unwrap()
}

fn wait(c: &Client, what: &str, pick: impl Fn(&Event) -> bool) -> Event {
    loop {
        match c.next_event(Duration::from_secs(10)) {
            Ok(e) if pick(&e) => return e,
            Ok(_) => {}
            Err(e) => panic!("no {what}: {e}"),
        }
    }
}

#[test]
fn two_clients_on_the_in_memory_server_connect_and_talk() {
    let (dir, server) = (Dir::new(), FakeSmp::new());
    let (alice, bob) = (client(&dir, "alice", &server), client(&dir, "bob", &server));
    let link = alice.create_invite().unwrap();
    bob.connect(&link).unwrap();
    let Event::FriendAdd(bob_at_alice) =
        wait(&alice, "friend", |e| matches!(e, Event::FriendAdd(_)))
    else {
        unreachable!()
    };
    wait(&bob, "friend", |e| matches!(e, Event::FriendAdd(_)));

    bob_at_alice.send("hello bob").unwrap();
    let Event::Message(m) = wait(
        &bob,
        "message",
        |e| matches!(e, Event::Message(m) if !m.mine()),
    ) else {
        unreachable!()
    };
    assert_eq!(m.text(), "hello bob");

    let mut ping = serde_json::Map::new();
    ping.insert("n".into(), 1.into());
    alice
        .send_data(bob_at_alice.chat(), "app.ping", ping.clone(), "ping")
        .unwrap();
    let Event::Data { tag, json, .. } = wait(&bob, "data", |e| matches!(e, Event::Data { .. }))
    else {
        unreachable!()
    };
    assert_eq!((tag.as_str(), json), ("app.ping", ping));
    alice.close();
    bob.close();
}

#[test]
fn a_message_sent_just_before_removing_a_friend_arrives_and_so_does_the_removal() {
    let (dir, server) = (Dir::new(), FakeSmp::new());
    let (alice, bob) = (client(&dir, "alice", &server), client(&dir, "bob", &server));
    bob.connect(&alice.create_invite().unwrap()).unwrap();
    let Event::FriendAdd(bob_at_alice) =
        wait(&alice, "friend", |e| matches!(e, Event::FriendAdd(_)))
    else {
        unreachable!()
    };
    wait(&bob, "friend", |e| matches!(e, Event::FriendAdd(_)));
    bob_at_alice.send("goodbye").unwrap();
    bob_at_alice.remove().unwrap();
    let Event::Message(m) = wait(
        &bob,
        "message",
        |e| matches!(e, Event::Message(m) if !m.mine()),
    ) else {
        unreachable!()
    };
    assert_eq!(m.text(), "goodbye");
    wait(&bob, "removal", |e| matches!(e, Event::FriendRemove(_)));
    alice.close();
    bob.close();
}

#[test]
fn a_client_hearing_deltas_gets_a_friends_change_and_a_whole_list_only_when_it_asks() {
    let (dir, server) = (Dir::new(), FakeSmp::new());
    let alice = client_with(&dir, "alice", &server, true);
    let bob = client(&dir, "bob", &server);
    bob.connect(&alice.create_invite().unwrap()).unwrap();
    let Event::FriendAdd(bob_at_alice) =
        wait(&alice, "friend", |e| matches!(e, Event::FriendAdd(_)))
    else {
        unreachable!()
    };
    bob_at_alice.set_alias("B").unwrap();
    let Event::FriendUpdate(f) = wait(&alice, "update", |e| {
        assert!(!matches!(e, Event::Raw(_)), "no whole list: {e:?}");
        matches!(e, Event::FriendUpdate(_))
    }) else {
        unreachable!()
    };
    assert_eq!(f.info().alias, "B");
    assert_eq!(alice.friend(f.id()).unwrap().info().alias, "B");
    alice.refresh().unwrap();
    wait(&alice, "whole list", |e| {
        matches!(e, Event::Raw(simplex_client::types::Event::Chats { .. }))
    });
    alice.close();
    bob.close();
}
