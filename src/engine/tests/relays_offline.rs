//! Offline from the relay checks: a start without a network says so once, and once again when it is back.

use super::relays_fake::{DEAD, LIVE, started};
use crate::engine::agent_port::AgentEvent;
use crate::engine::clock::Clock;
use crate::types::Event;

fn offline(out: &[Event]) -> Vec<bool> {
    let o = out.iter();
    o.filter_map(|e| match e {
        Event::Offline(o) => Some(*o),
        _ => None,
    })
    .collect()
}

#[test]
fn a_start_where_no_relay_answers_says_offline_once_and_online_when_a_check_reaches_one() {
    let (mut t, fake) = started(&[LIVE, DEAD]);
    assert!(offline(&t.relay_answers(&fake, LIVE, false)).is_empty());
    assert_eq!(offline(&t.relay_answers(&fake, DEAD, false)), [true]);
    assert!(
        t.e.next_relay_wake().is_some(),
        "checks again while offline"
    );
    t.relay_time(181);
    t.relay_answers(&fake, LIVE, false);
    assert!(
        offline(&t.relay_answers(&fake, DEAD, false)).is_empty(),
        "said once"
    );
    t.relay_time(181);
    t.relay_answers(&fake, DEAD, false);
    assert_eq!(offline(&t.relay_answers(&fake, LIVE, true)), [false]);
    assert_eq!(
        t.e.next_relay_wake(),
        Some(t.clock.wall_ms() + 180_000),
        "DEAD is disabled"
    );
}

#[test]
fn a_lone_relay_that_does_not_answer_at_the_start_says_offline() {
    let (mut t, fake) = started(&[LIVE]);
    assert_eq!(offline(&t.relay_answers(&fake, LIVE, false)), [true]);
}

#[test]
fn a_relay_the_agent_brings_up_after_an_offline_start_says_online_once() {
    let (mut t, fake) = started(&[LIVE]);
    t.relay_answers(&fake, LIVE, false);
    let out = t.agent(AgentEvent::Up {
        server: "127.0.0.1".into(),
    });
    assert_eq!(offline(&out), [false]);
    t.relay_time(181);
    assert!(fake.asked().len() == 1, "no more checks once online");
}

#[test]
fn a_check_reaching_no_relay_while_the_agent_has_one_up_is_not_offline() {
    let (mut t, fake) = started(&[LIVE, DEAD]);
    t.agent(AgentEvent::Up {
        server: "127.0.0.1".into(),
    });
    t.relay_answers(&fake, LIVE, false);
    assert!(offline(&t.relay_answers(&fake, DEAD, false)).is_empty());
}

#[test]
fn a_check_that_hears_nothing_by_its_deadline_says_offline() {
    let (mut t, _) = started(&[LIVE, DEAD]);
    assert_eq!(offline(&t.relay_time(91)), [true]);
}
