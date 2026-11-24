//! `Client` over the engine's fake agent.

mod calls;
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
