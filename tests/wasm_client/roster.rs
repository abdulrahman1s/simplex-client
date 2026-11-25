//! `rosterDeltas`: changes heard as `friendUpdate` and the adds and removes, `refresh()` asks for the whole list, and the option is a boolean.

use serde_json::json;
use simplex_client::types::model::Friend;
use simplex_client::types::{Event, Request};
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::support::{Heard, fake_tunnel, from_js, settle, sleep, start};

fn bob(name: &str) -> Friend {
    Friend {
        id: 4,
        name: name.into(),
        ready: true,
        ..Friend::default()
    }
}

fn changed(friends: Vec<Friend>, removed_friends: Vec<i64>) -> Event {
    Event::ChatsChanged {
        friends,
        groups: Vec::new(),
        removed_friends,
        removed_groups: Vec::new(),
        requests: None,
    }
}

#[test]
async fn with_roster_deltas_a_friends_changes_are_heard_one_by_one() {
    fake_tunnel("silent");
    let client = start(json!({ "rosterDeltas": true })).unwrap();
    sleep(20.0).await;
    let (added, updated, removed) = (
        Heard::on(&client, "friendAdd"),
        Heard::on(&client, "friendUpdate"),
        Heard::on(&client, "friendRemove"),
    );
    client.inject(changed(vec![bob("bob")], Vec::new()));
    client.inject(changed(vec![bob("bobby")], Vec::new()));
    assert_eq!(client.friends().size(), 1);
    client.inject(changed(Vec::new(), vec![4]));
    let names = |h: &Heard| -> Vec<_> { h.all().iter().map(|f| f["name"].clone()).collect() };
    assert_eq!(names(&added), [json!("bob")]);
    assert_eq!(names(&updated), [json!("bobby")]);
    assert_eq!(names(&removed), [json!("bobby")]);
    assert_eq!(client.friends().size(), 0);
    client.tap(true);
    settle(client.refresh().into()).await.unwrap();
    assert_eq!(client.tap(false), [Request::Refresh]);
    client.close();
}

#[test]
async fn roster_deltas_that_is_not_a_boolean_is_a_bad_option() {
    fake_tunnel("silent");
    let Err(e) = start(json!({ "rosterDeltas": "yes" })) else {
        panic!("started")
    };
    assert_eq!(from_js(&e)["code"], "bad_option");
}
