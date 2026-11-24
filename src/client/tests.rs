//! `Client` over the engine's fake agent: answers, failures, timeouts, overflow and closing.

use std::time::Duration;

use crossbeam_channel::{Sender, unbounded};

use super::builder::ClientBuilder;
use super::{Client, Error};
use crate::engine::FakeClock;
use crate::engine::agent_port::{AgentEvent, AgentFailure, ConnId};
use crate::engine::tests::{Dir, FakeAgent, KEY};
use crate::store::StoreError;
use crate::types::model::ChatRef;
use crate::types::{ErrorCode, Event, RequestKind};

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
fn start_reports_ready_with_the_profile_before_returning() {
    let dir = Dir::new();
    let (client, _smp) = start(builder(&dir), FakeAgent::default());
    assert_eq!(client.profile().name, "alice");
    wait_for(
        &client,
        |e| matches!(e, Event::Ready(p) if p.name == "alice"),
    );
}

#[test]
fn create_invite_returns_the_link_the_relay_made() {
    let dir = Dir::new();
    let mut agent = FakeAgent::default();
    agent.on_invitation = vec![invitation()];
    let (client, _smp) = start(builder(&dir), agent);
    assert_eq!(client.create_invite().unwrap(), LINK);
}

#[test]
fn each_create_invite_gets_its_own_link() {
    let dir = Dir::new();
    let mut agent = FakeAgent::default();
    agent.on_invitation = vec![invitation_on(1, LINK), invitation_on(2, "simplex:/second")];
    let (client, _smp) = start(builder(&dir), agent);
    assert_eq!(client.create_invite().unwrap(), LINK);
    assert_eq!(client.create_invite().unwrap(), "simplex:/second");
}

#[test]
fn a_client_is_shared_between_threads() {
    fn is<T: Send + Sync + Clone + 'static>() {}
    is::<Client>();
}

#[test]
fn a_refused_request_is_the_engines_code() {
    let dir = Dir::new();
    let mut agent = FakeAgent::default();
    agent.fail_next = Some(AgentFailure::Network("timeout".into()));
    let (client, _smp) = start(builder(&dir), agent);
    match client.create_invite() {
        Err(Error::Failed { request, code, .. }) => {
            assert_eq!(
                (request, code),
                (RequestKind::CreateInvite, ErrorCode::NoConnection)
            );
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_answer_that_comes_after_the_timeout_goes_to_events() {
    let dir = Dir::new();
    let (client, smp) = start(builder(&dir), FakeAgent::default());
    assert!(matches!(client.create_invite(), Err(Error::Timeout)));
    smp.send(invitation()).unwrap();
    wait_for(&client, |e| *e == Event::Invite(LINK.into()));
}

#[test]
fn a_failure_for_a_keyed_call_reaches_that_caller() {
    let dir = Dir::new();
    let (client, _smp) = start(builder(&dir), FakeAgent::default());
    assert!(matches!(
        client.members(42),
        Err(Error::Failed {
            request: RequestKind::Members,
            ..
        })
    ));
    assert!(matches!(
        client.history(ChatRef::Direct(7), None, 10),
        Err(Error::Failed {
            request: RequestKind::History,
            ..
        })
    ));
}

#[test]
fn unread_events_do_not_stop_answers() {
    let dir = Dir::new();
    let mut b = builder(&dir);
    b.events_cap = 1;
    let mut agent = FakeAgent::default();
    agent.on_invitation = vec![invitation()];
    let (client, _smp) = start(b, agent);
    for _ in 0..50 {
        client.request(crate::types::Request::Refresh).unwrap();
    }
    assert_eq!(client.create_invite().unwrap(), LINK);
}

#[test]
fn close_ends_events_and_later_calls_fail_closed() {
    let dir = Dir::new();
    let (client, _smp) = start(builder(&dir), FakeAgent::default());
    let other = client.clone();
    client.close();
    assert!(
        other.events().count() < 16,
        "the queued events drain, then the iterator ends"
    );
    assert!(matches!(
        other.send(ChatRef::Direct(1), "hi"),
        Err(Error::Closed)
    ));
    assert!(matches!(other.create_invite(), Err(Error::Closed)));
}

#[test]
fn a_wrong_key_is_a_store_error_and_the_store_reopens_after_drop() {
    let dir = Dir::new();
    let (client, smp) = start(builder(&dir), FakeAgent::default());
    drop((client, smp));
    let (smp, rx) = unbounded::<AgentEvent>();
    let wrong = builder(&dir).key("another key");
    let clock = Box::new(FakeClock::new(0));
    let r = wrong.start_with(clock, move |_, _| (FakeAgent::default(), rx));
    assert!(
        matches!(r, Err(Error::Store(StoreError::WrongKey))),
        "{r:?}"
    );
    drop(smp);
    let (client, _smp) = start(builder(&dir), FakeAgent::default());
    assert_eq!(client.profile().name, "alice");
}
