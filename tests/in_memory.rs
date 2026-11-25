//! Two clients on the in-memory SMP server (feature `testing`): they connect, talk and send application data, with no network.
#![cfg(feature = "testing")]
#![allow(clippy::unwrap_used, reason = "tests")]

use std::path::PathBuf;
use std::time::Duration;

use simplex_client::agent::testing::{Dir, FakeSmp};
use simplex_client::{Client, Event};

fn client(dir: &Dir, name: &str, server: &std::sync::Arc<FakeSmp>) -> Client {
    let folder: PathBuf = dir.0.join(name);
    Client::builder(folder)
        .name(name)
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
