//! `Client` over the engine's fake agent.

mod calls;
mod key;
mod limits;
mod objects;

use std::time::Duration;

use crossbeam_channel::{Sender, unbounded};

use super::builder::ClientBuilder;
use super::{Client, Event};
use crate::engine::FakeClock;
use crate::engine::agent_port::{AgentEvent, ConnId};
use crate::engine::tests::{Dir, FakeAgent, KEY};

const LINK: &str = "simplex:/invitation#/?v=2-7&smp=fake";

fn builder(dir: &Dir) -> ClientBuilder {
    Client::builder(&dir.0)
        .name("alice")
        .key(KEY)
        .log(std::io::sink())
        .timeout(Duration::from_millis(300))
}

/// A client over `agent`, and the sender of the agent's "network" events.
fn start(b: ClientBuilder, agent: FakeAgent) -> (Client, Sender<AgentEvent>) {
    let (smp, rx) = unbounded();
    let clock = Box::new(FakeClock::new(1_795_000_000_000));
    let client = b.start_with(clock, move |_, _| (agent, rx)).unwrap();
    (client, smp)
}

fn invitation() -> AgentEvent {
    invitation_on(1, LINK)
}

fn invitation_on(conn: i64, link: &str) -> AgentEvent {
    AgentEvent::Invitation {
        conn: ConnId(conn),
        full: link.into(),
        short: None,
    }
}

fn wait_for(client: &Client, pick: impl Fn(&Event) -> bool) -> Event {
    loop {
        let e = client.next_event(Duration::from_secs(5)).unwrap();
        if pick(&e) {
            return e;
        }
    }
}

#[test]
fn a_second_client_on_the_same_store_fails_while_the_first_runs_and_starts_after_it_closes() {
    let dir = Dir::new();
    let (first, _net) = start(builder(&dir), FakeAgent::default());
    let (smp, rx) = unbounded::<AgentEvent>();
    let clock = Box::new(FakeClock::new(1_795_000_000_000));
    let second = builder(&dir).start_with(clock, move |_, _| (FakeAgent::default(), rx));
    let Err(super::Error::Locked(locked)) = second else {
        panic!("the second client started")
    };
    assert_eq!(locked, dir.0);
    drop(smp);
    first.close();
    let (again, _net) = start(builder(&dir), FakeAgent::default());
    again.close();
}

#[test]
fn a_send_returns_the_handle_its_status_is_told_against() {
    let dir = Dir::new();
    let (client, _net) = start(builder(&dir), FakeAgent::default());
    let handle = client
        .send(crate::types::model::ChatRef::Direct(404), "hi")
        .unwrap();
    let e = wait_for(&client, |e| matches!(e, Event::SendStatus { .. }));
    let Event::SendStatus {
        handle: h, status, ..
    } = e
    else {
        unreachable!()
    };
    assert_eq!(h, handle);
    assert!(matches!(
        status,
        crate::types::model::SendStatus::Refused { .. }
    ));
    let other = client
        .send(crate::types::model::ChatRef::Direct(404), "again")
        .unwrap();
    assert_ne!(other, handle);
    client.close();
}
