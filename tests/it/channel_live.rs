//! Channels against official simplex-chat 7.1.4 binaries on local servers only: an official
//! CLI run with `--relay` relays, other official CLIs own or subscribe. Set
//! `SIMPLEX_TEST_SMP` (the local smp-server), `SIMPLEX_TEST_RELAY` (the relay CLI's `/r#`
//! address), `SIMPLEX_TEST_CLI_OWNER` and `SIMPLEX_TEST_CLI_SUB` (`ws://127.0.0.1:<port>` of
//! two official CLIs started with `-p <port>`, already with a profile) and, for the two-relay
//! case, `SIMPLEX_TEST_RELAY2` (a second relay CLI's address), then
//! `cargo test --test it channel_live -- --ignored --test-threads 1`. With
//! `SIMPLEX_TEST_LOG=<dir>` each client's engine log goes to `<dir>/<name>.log`. Run it all
//! where only local servers are reachable (a network namespace with only `lo`): a relay makes
//! its address on a preset server unless told `--relay-address-server`.
#![allow(clippy::disallowed_methods, reason = "the test waits on real time")]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use serde_json::{Map, Value, json};
use simplex_client::{Author, ChatRelay, Client, Event, Group, MessageAuthor, types};

use super::cli_ws::{Cli, items};

const WAIT: Duration = Duration::from_secs(90);
const TAG: &str = "zz.app";
// A relay replays only posts with text or a file (Internal.hs `processContentItem`), so the
// data carries fallback text.

fn env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"))
}

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("simplex-channel-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        Self(dir)
    }
}

impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn start(dir: &Dir, name: &str) -> Client {
    let mut b = Client::builder(&dir.0);
    // `SIMPLEX_TEST_LOG=<dir>`: each client's engine log goes to `<dir>/<name>.log`.
    if let Ok(logs) = std::env::var("SIMPLEX_TEST_LOG") {
        let f = std::fs::File::create(PathBuf::from(logs).join(format!("{name}.log"))).unwrap();
        b = b.log(f);
    }
    b.name(name)
        .key("key")
        .relay(env("SIMPLEX_TEST_SMP"))
        .data_tag(TAG)
        .start()
        .unwrap()
}

fn wait<T>(c: &Client, what: &str, pick: impl Fn(&Event) -> Option<T>) -> T {
    let end = Instant::now() + WAIT;
    loop {
        let left = end.saturating_duration_since(Instant::now());
        let ev = c
            .next_event(left)
            .unwrap_or_else(|e| panic!("waiting for {what}: {e}"));
        if let Event::Raw(types::Event::Failed {
            request,
            code,
            detail,
        }) = &ev
        {
            panic!("{request:?} failed while waiting for {what}: {code} {detail}");
        }
        if let Some(t) = pick(&ev) {
            return t;
        }
    }
}

fn data(n: i64) -> Map<String, Value> {
    // Keys out of order: relays write them back sorted.
    let Value::Object(m) = json!({"zeta": n, "alpha": {"b": 2, "a": 1}}) else {
        unreachable!()
    };
    m
}

/// The crate's channel once every relay is active, with the channel's state.
fn active_channel(c: &Client, g: &Group) -> types::model::ChannelInfo {
    let end = Instant::now() + WAIT;
    loop {
        let ch = c.group(g.id()).and_then(|g| g.info().channel.clone());
        if let Some(ch) = ch.filter(|ch| {
            !ch.relays.is_empty()
                && ch
                    .relays
                    .iter()
                    .all(|r| r.status == Some(types::model::RelayStatus::Active))
        }) {
            return ch;
        }
        assert!(
            Instant::now() < end,
            "the channel's relays did not become active"
        );
        let _ = c.next_event(Duration::from_millis(500));
    }
}

/// The crate subscriber's joined channel.
fn joined(c: &Client) -> Group {
    wait(c, "the channel joined", |e| match e {
        Event::GroupAdd(g) if g.info().channel.is_some() => Some(g.clone()),
        _ => None,
    })
}

fn data_in(c: &Client, n: i64) -> Map<String, Value> {
    wait(c, "channel data", |e| match e {
        Event::Data {
            member: None,
            tag,
            json,
            ..
        } if tag == TAG && json.get("zeta") == Some(&json!(n)) => Some(json.clone()),
        _ => None,
    })
}

/// A CLI subscriber's received post with this `zeta`, as `(chatDir, content)`.
fn cli_got(cli: &mut Cli, n: i64) -> (String, Value) {
    cli.wait("a channel post", WAIT, |r| {
        (r["type"] == "newChatItems")
            .then(|| {
                items(r)
                    .into_iter()
                    .find(|(_, c)| c["msgContent"]["zeta"] == json!(n))
            })
            .flatten()
    })
}

/// What became of a send the engine refused: its detail.
fn refusal(c: &Client, h: simplex_client::client::SendHandle) -> String {
    let status = wait(c, "the send's outcome", |e| match e {
        Event::SendStatus { handle, status, .. } if *handle == h => Some(status.clone()),
        _ => None,
    });
    let types::model::SendStatus::Refused { detail, .. } = status else {
        panic!("not refused: {status:?}")
    };
    detail
}

fn cli_group_id(resp: &Value) -> i64 {
    resp["groupInfo"]["groupId"].as_i64().unwrap()
}

#[test]
#[ignore = "needs local servers, an official relay and CLIs; see the module docs"]
fn an_official_owner_s_channel_reaches_a_crate_subscriber_with_history_and_no_pq() {
    let mut owner = Cli::connect(&env("SIMPLEX_TEST_CLI_OWNER"));
    let relay = env("SIMPLEX_TEST_RELAY");
    assert_eq!(
        owner.cmd(&format!("/relays name=relay {relay}"))["type"],
        "cmdOk"
    );
    let name = format!("o{}", std::process::id());
    let made = owner.cmd(&format!("/public group relays=1 #{name}"));
    assert_eq!(made["type"], "publicGroupCreated", "{made}");
    let gid = cli_group_id(&made);
    let link = made["groupInfo"]["groupProfile"]["publicGroup"]["groupLink"]
        .as_str()
        .unwrap()
        .to_owned();
    owner.wait("the relay active", WAIT, |r| {
        (r["type"] == "groupLinkDataUpdated").then_some(())
    });
    // History: posted before the subscriber joins.
    for n in 1..=3 {
        let sent = owner.cmd(&format!(
            "/_send #{gid}(as_group=on) json [{{\"msgContent\":{{\"type\":\"{TAG}\",\"text\":\"post\",\"zeta\":{n},\"alpha\":{{\"b\":2,\"a\":1}}}}}}]"
        ));
        assert_eq!(sent["type"], "newChatItems", "{sent}");
    }

    let dir = Dir::new("sub");
    let sub = start(&dir, "sub");
    sub.connect(&link).unwrap();
    let g = joined(&sub);
    for n in 1..=3 {
        assert_eq!(data_in(&sub, n), data(n), "history post {n}");
    }
    owner.cmd(&format!(
        "/_send #{gid}(as_group=on) json [{{\"msgContent\":{{\"type\":\"{TAG}\",\"text\":\"post\",\"zeta\":4,\"alpha\":{{\"b\":2,\"a\":1}}}}}}]"
    ));
    assert_eq!(data_in(&sub, 4), data(4));
    owner.cmd(&format!(
        "/_send #{gid}(as_group=on) text hello subscribers"
    ));
    let m = wait(&sub, "a text post", |e| match e {
        Event::Message(m) if m.text() == "hello subscribers" => Some(m.clone()),
        _ => None,
    });
    assert!(matches!(m.author(), Author::Channel(id) if id == g.id()));
    assert!(matches!(m.info().author, MessageAuthor::Channel(_)));

    let ch = sub.group(g.id()).unwrap().info().channel.clone().unwrap();
    assert!(!ch.owner && ch.use_relays);
    assert!(ch.relays.iter().any(|r| r.connected));
    assert!(
        ch.relays.iter().all(|r| !r.pq),
        "relay legs have no PQ KEM: {ch:?}"
    );
    assert_eq!(
        sub.group(g.id()).unwrap().info().role,
        types::model::MemberRole::Observer
    );

    let h = g.send("an observer's post").unwrap();
    assert_eq!(refusal(&sub, h), "groupUserRole: required role author");
    sub.request(types::Request::LeaveGroup(g.id())).unwrap();
    sub.close();
}

#[test]
#[ignore = "needs local servers, an official relay and CLIs; see the module docs"]
fn a_crate_owner_s_channel_reaches_an_official_subscriber_who_may_not_post() {
    let dir = Dir::new("owner");
    let owner = start(&dir, "owner");
    owner
        .set_chat_relays(vec![ChatRelay {
            address: env("SIMPLEX_TEST_RELAY"),
            name: "relay".into(),
        }])
        .unwrap();
    let g = owner
        .create_channel(&format!("c{}", std::process::id()))
        .unwrap();
    let ch = active_channel(&owner, &g);
    assert!(ch.owner && ch.relays.iter().all(|r| !r.pq), "{ch:?}");
    owner.send_data(g.chat(), TAG, data(1), "post").unwrap();
    owner
        .send_signed_data(g.chat(), TAG, data(2), "post")
        .unwrap();

    let mut sub = Cli::connect(&env("SIMPLEX_TEST_CLI_SUB"));
    let joined = sub.cmd(&format!("/c {}", ch.link));
    assert_eq!(joined["type"], "startedConnectionToGroup", "{joined}");
    let gid = cli_group_id(&joined);
    let e2ee = sub.wait("the e2ee notice", WAIT, |r| {
        items(r)
            .into_iter()
            .find(|(_, c)| c["type"] == "rcvGroupE2EEInfo")
            .map(|(_, c)| c)
    });
    assert_eq!(
        e2ee["e2eeInfo"],
        json!({"public": true, "pqEnabled": false})
    );
    for n in 1..=2 {
        let (dir, content) = cli_got(&mut sub, n);
        // A signed post is forwarded naming the owner, and still shows as the channel's.
        assert_eq!(dir, "channelRcv", "history post {n}");
        assert_eq!(content["msgContent"]["alpha"], json!({"a": 1, "b": 2}));
    }
    owner.send_data(g.chat(), TAG, data(3), "post").unwrap();
    assert_eq!(cli_got(&mut sub, 3).0, "channelRcv");

    let refused = sub.cmd(&format!("/_send #{gid} text from a subscriber"));
    assert_eq!(refused["type"], "chatCmdError", "{refused}");
    assert_eq!(
        refused["chatError"]["errorType"]["type"], "groupUserRole",
        "{refused}"
    );
    owner.close();
}

#[test]
#[ignore = "needs local servers, an official relay and CLIs; see the module docs"]
fn two_crate_clients_share_a_channel_through_an_official_relay() {
    let (od, sd) = (Dir::new("o2"), Dir::new("s2"));
    let owner = start(&od, "owner2");
    owner
        .set_chat_relays(vec![ChatRelay {
            address: env("SIMPLEX_TEST_RELAY"),
            name: "relay".into(),
        }])
        .unwrap();
    let g = owner
        .create_channel(&format!("d{}", std::process::id()))
        .unwrap();
    let ch = active_channel(&owner, &g);
    owner.send_data(g.chat(), TAG, data(1), "post").unwrap();
    let sub = start(&sd, "sub2");
    sub.connect(&ch.link).unwrap();
    let sg = joined(&sub);
    assert_eq!(data_in(&sub, 1), data(1), "history");
    owner
        .send_signed_data(g.chat(), TAG, data(2), "post")
        .unwrap();
    assert_eq!(data_in(&sub, 2), data(2), "a signed post");
    let sch = sub.group(sg.id()).unwrap().info().channel.clone().unwrap();
    assert!(sch.relays.iter().all(|r| !r.pq) && ch.relays.iter().all(|r| !r.pq));
    let h = sub.send_data(sg.chat(), TAG, data(9), "post").unwrap();
    assert_eq!(refusal(&sub, h), "groupUserRole: required role author");
    owner.close();
    sub.close();
}

/// The JSON of a data post whose unsigned body is `len` bytes.
fn data_of_len(len: usize) -> Map<String, Value> {
    let body = |n: usize| {
        let Value::Object(m) = json!({"zeta": 99, "pad": "x".repeat(n)}) else {
            unreachable!()
        };
        let c = simplex_client::chat::app_data::content(TAG, &m, "big");
        let msg = simplex_client::chat::ChatMessage::new(simplex_client::chat::ChatEvent::MsgNew(
            simplex_client::chat::container::MsgContainer::simple(c),
        ))
        .unwrap();
        (m, msg.to_json().unwrap().len())
    };
    let (_, base) = body(0);
    let (m, got) = body(len - base);
    assert_eq!(got, len);
    m
}

#[test]
#[ignore = "needs local servers, two official relays (SIMPLEX_TEST_RELAY2) and CLIs; see the module docs"]
fn a_channel_on_two_relays_replays_the_last_100_posts_once_each_as_they_now_are() {
    let (od, sd) = (Dir::new("o3"), Dir::new("s3"));
    let owner = start(&od, "owner3");
    let relays = ["SIMPLEX_TEST_RELAY", "SIMPLEX_TEST_RELAY2"]
        .iter()
        .enumerate()
        .map(|(i, v)| ChatRelay {
            address: env(v),
            name: format!("relay{i}"),
        })
        .collect();
    owner.set_chat_relays(relays).unwrap();
    let g = owner
        .create_channel(&format!("e{}", std::process::id()))
        .unwrap();
    let ch = active_channel(&owner, &g);
    assert_eq!(ch.relays.len(), 2);
    for n in 1..=105 {
        g.send(&format!("post {n}")).unwrap();
    }
    wait(&owner, "the owner's last post", |e| match e {
        Event::Message(m) if m.mine() && m.text() == "post 105" => Some(()),
        _ => None,
    });
    let find = |text: &str| {
        g.history(200)
            .unwrap()
            .into_iter()
            .find(|m| m.text() == text)
            .unwrap()
    };
    find("post 104").edit("post 104, edited").unwrap();
    find("post 105").delete().unwrap();
    owner.send_data(g.chat(), TAG, data(1), "post").unwrap();
    // The largest signed post: its body as sent (with the 15 bytes of `"asGroup":true,`) and
    // the 69 bytes of its one signature; one byte more is refused before it goes.
    let max = simplex_client::limits::MAX_ENCODED_MSG_LENGTH;
    let over = owner
        .send_signed_data(g.chat(), TAG, data_of_len(max - 69 - 15 + 1), "big")
        .unwrap();
    assert_eq!(refusal(&owner, over), "signed message too large");
    owner
        .send_signed_data(g.chat(), TAG, data_of_len(max - 69 - 15), "big")
        .unwrap();
    std::thread::sleep(Duration::from_secs(3));

    let sub = start(&sd, "sub3");
    sub.connect(&ch.link).unwrap();
    let sg = joined(&sub);
    let mut texts = Vec::new();
    let mut datas = Vec::new();
    let end = Instant::now() + WAIT;
    while datas.len() < 2 && Instant::now() < end {
        match sub.next_event(Duration::from_secs(5)) {
            Ok(Event::Message(m)) if matches!(m.info().author, MessageAuthor::Channel(_)) => {
                texts.push((m.text().to_owned(), m.info().edited));
            }
            Ok(Event::Data { json, .. }) => datas.push(json),
            Ok(_) => {}
            Err(_) => break,
        }
    }
    // Exactly the last 100 still there, in order: 98 texts and the 2 data posts after them
    // (the deleted post is not one); the edit as it is now, not marked edited.
    let want: Vec<(String, bool)> = (7..=104)
        .map(|n| match n {
            104 => ("post 104, edited".to_owned(), false),
            n => (format!("post {n}"), false),
        })
        .collect();
    assert_eq!(texts, want);
    let zetas: Vec<_> = datas.iter().map(|d| d["zeta"].clone()).collect();
    assert_eq!(
        datas.len(),
        2,
        "each data post once, though two relays bring it: {zetas:?}"
    );
    assert_eq!(
        datas[1]["pad"].as_str().map(str::len),
        data_of_len(max - 69 - 15)["pad"].as_str().map(str::len)
    );

    let both = Instant::now() + WAIT;
    let sch = loop {
        let sch = sub.group(sg.id()).unwrap().info().channel.clone().unwrap();
        if sch.relays.iter().filter(|r| r.connected).count() == 2 {
            break sch;
        }
        assert!(Instant::now() < both, "both relays connected: {sch:?}");
        let _ = sub.next_event(Duration::from_millis(200));
    };
    assert!(sch.relays.iter().all(|r| !r.pq));
    sub.request(types::Request::LeaveGroup(sg.id())).unwrap();
    let left = Instant::now() + WAIT;
    while sub
        .group(sg.id())
        .is_some_and(|g| g.info().membership != types::model::Membership::Left)
    {
        assert!(Instant::now() < left, "the subscriber did not leave");
        let _ = sub.next_event(Duration::from_millis(200));
    }
    owner.close();
    sub.close();
}
