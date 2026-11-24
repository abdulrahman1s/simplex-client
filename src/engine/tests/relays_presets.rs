//! With no relay named, the presets the agent uses are checked like own relays (the old engine tested its presets too).

use super::relays::{disabled_calls, servers};
use super::relays_fake::started;
use crate::engine::presets;
use crate::types::Event;

fn in_use() -> Vec<String> {
    let mut d: Vec<String> = presets::default_addresses().map(str::to_owned).collect();
    d.sort();
    d
}

#[test]
fn with_no_relay_named_the_presets_that_take_queues_are_checked_and_no_other() {
    let (_, fake) = started(&[]);
    assert_eq!(servers(&fake), in_use());
    assert!(in_use().len() < presets::all().len());
}

#[test]
fn a_silent_preset_is_disabled_for_new_queues() {
    let (mut t, fake) = started(&[]);
    let (dead, rest) = in_use()
        .split_first()
        .map(|(d, r)| (d.clone(), r.to_vec()))
        .unwrap();
    t.relay_answers(&fake, &dead, false);
    for r in &rest {
        t.relay_answers(&fake, r, true);
    }
    assert_eq!(disabled_calls(&t), vec![vec![dead]]);
}

#[test]
fn a_start_on_the_presets_without_a_network_says_offline() {
    let (mut t, fake) = started(&[]);
    let mut out = Vec::new();
    for r in in_use() {
        out.extend(t.relay_answers(&fake, &r, false));
    }
    assert_eq!(
        out.iter()
            .filter(|e| matches!(e, Event::Offline(true)))
            .count(),
        1
    );
    assert!(disabled_calls(&t).is_empty());
}

#[test]
fn named_relays_that_do_not_parse_leave_nothing_to_check_and_never_the_presets() {
    let (_, fake) = started(&["smp://not a relay"]);
    assert!(fake.asked().is_empty());
}
