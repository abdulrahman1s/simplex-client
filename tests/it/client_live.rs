//! Two clients through a real smp-server. Start a local smp-server and run
//! `SIMPLEX_TEST_SMP=smp://<fingerprint>[:<password>]@127.0.0.1:<port> cargo test --test it client_live -- --ignored`.
#![allow(
    clippy::disallowed_methods,
    reason = "the test waits on real time and times the shutdown"
)]

use std::path::PathBuf;
use std::time::{Duration, Instant};

use simplex_client::{ChatMessage, Client, Delivery, Event, Friend};

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
        if let Event::Failed {
            request,
            code,
            detail,
        } = &ev
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
        Event::Chats { friends, .. } => friends.iter().find(|f| f.ready).cloned(),
        _ => None,
    })
}

fn text(c: &Client, what: &str, text: &str) -> ChatMessage {
    wait(c, what, |e| match e {
        Event::Messages(ms) => ms.iter().find(|m| !m.mine() && m.text() == text).cloned(),
        _ => None,
    })
}

fn delivered(c: &Client, text: &str) {
    wait(c, "a delivery receipt", |e| match e {
        Event::Updated(ms) => ms
            .iter()
            .any(|m| m.text() == text && m.delivery == Delivery::Delivered)
            .then_some(()),
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
    assert_eq!((for_a.name.as_str(), for_b.name.as_str()), ("bob", "alice"));

    a.send(for_a.chat(), "hello bob").unwrap();
    let got = text(&b, "alice's text", "hello bob");
    delivered(&a, "hello bob");
    b.reply(&got, "hi alice").unwrap();
    text(&a, "bob's reply", "hi alice");

    close(b);
    let b = start(&dir_b, "bob");
    a.send(for_a.chat(), "while you restarted").unwrap();
    text(&b, "the text after the restart", "while you restarted");
    delivered(&a, "while you restarted");
    b.send(for_b.chat(), "welcome back").unwrap();
    text(&a, "bob's text after the restart", "welcome back");

    close(a);
    close(b);
}
