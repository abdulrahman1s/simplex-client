//! The store key: from a buffer that wipes itself, the same store as from a string, and never in a `Debug` of the config.

use crossbeam_channel::unbounded;
use zeroize::Zeroizing;

use super::{builder, start};
use crate::client::Error;
use crate::engine::FakeClock;
use crate::engine::agent_port::AgentEvent;
use crate::engine::tests::{Dir, FakeAgent, KEY};
use crate::store::StoreError;

#[test]
fn a_zeroizing_key_opens_the_store_a_string_key_made() {
    let dir = Dir::new();
    let (client, _net) = start(builder(&dir), FakeAgent::default());
    client.close();
    let b = builder(&dir).key_zeroizing(Zeroizing::new(KEY.to_owned()));
    let (client, _net) = start(b, FakeAgent::default());
    assert_eq!(client.profile().name, "alice");
}

#[test]
fn an_empty_zeroizing_key_is_refused_like_a_missing_one() {
    let dir = Dir::new();
    let (smp, rx) = unbounded::<AgentEvent>();
    let b = builder(&dir).key_zeroizing(Zeroizing::new(String::new()));
    let r = b.start_with(Box::new(FakeClock::new(0)), move |_, _| {
        (FakeAgent::default(), rx)
    });
    assert!(matches!(r, Err(Error::Store(StoreError::Invalid(_)))));
    drop(smp);
}

#[test]
fn the_configs_debug_leaves_the_key_out() {
    let dir = Dir::new();
    let b = builder(&dir).key("hunter2 hunter2");
    let shown = format!("{:?}", b.config);
    assert!(!shown.contains("hunter2"), "{shown}");
    assert!(shown.contains("<redacted>"));
}
