//! Two clients through a real smp-server. Start a local smp-server and run
//! `SIMPLEX_TEST_SMP=smp://<fingerprint>[:<password>]@127.0.0.1:<port> cargo test --test it client_live -- --ignored`.
#![allow(
    clippy::disallowed_methods,
    reason = "the test waits on real time and times the shutdown"
)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use simplex_client::{Client, Delivery, Event, Friend, Message, types};

const WAIT: Duration = Duration::from_secs(60);

fn relay() -> String {
    let Ok(addr) = std::env::var("SIMPLEX_TEST_SMP") else {
        panic!("SIMPLEX_TEST_SMP is not set")
    };
    addr
}

struct Dir(PathBuf);

impl Dir {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("simplex-live-{name}-{}", std::process::id()));
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
    Client::builder(&dir.0)
        .name(name)
        .key("key")
        .relay(relay())
        .start()
        .unwrap()
}

/// The first later event `pick` takes; the ones before it are dropped.
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
            panic!("{request:?} failed: {code} {detail}");
        }
        if let Some(t) = pick(&ev) {
            return t;
        }
    }
}

fn ready_friend(c: &Client) -> Friend {
    wait(c, "a ready friend", |e| match e {
        Event::FriendAdd(f) => Some(f.clone()),
        _ => None,
    })
}

fn text(c: &Client, what: &str, text: &str) -> Message {
    wait(c, what, |e| match e {
        Event::Message(m) if !m.mine() && m.text() == text => Some(m.clone()),
        _ => None,
    })
}

fn delivered(c: &Client, text: &str) {
    wait(c, "a delivery receipt", |e| match e {
        Event::MessageUpdate(m) => {
            (m.text() == text && m.info().delivery == Delivery::Delivered).then_some(())
        }
        _ => None,
    });
}

/// Must close within 5 seconds.
fn close(c: Client) {
    let started = Instant::now();
    c.close();
    assert!(started.elapsed() < Duration::from_secs(5), "slow close");
}

#[test]
#[ignore = "needs a local smp-server; set SIMPLEX_TEST_SMP"]
fn two_clients_connect_exchange_messages_with_receipts_and_survive_a_restart() {
    let (dir_a, dir_b) = (Dir::new("alice"), Dir::new("bob"));
    let a = start(&dir_a, "alice");
    let b = start(&dir_b, "bob");

    let link = a.create_invite().unwrap();
    b.connect(&link).unwrap();
    let (for_a, for_b) = (ready_friend(&a), ready_friend(&b));
    assert_eq!((for_a.name(), for_b.name()), ("bob", "alice"));

    for_a.send("hello bob").unwrap();
    let got = text(&b, "alice's text", "hello bob");
    delivered(&a, "hello bob");
    got.reply("hi alice").unwrap();
    text(&a, "bob's reply", "hi alice");

    close(b);
    let b = start(&dir_b, "bob");
    for_a.send("while you restarted").unwrap();
    text(&b, "the text after the restart", "while you restarted");
    delivered(&a, "while you restarted");
    let for_b = b.friend(for_b.id()).unwrap();
    for_b.send("welcome back").unwrap();
    text(&a, "bob's text after the restart", "welcome back");

    close(a);
    close(b);
}
