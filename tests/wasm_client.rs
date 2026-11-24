//! The browser client under node with a fake tunnel: start, requests, events, snapshots, and a refused tunnel.
#![cfg(all(target_arch = "wasm32", feature = "wasm"))]
#![allow(clippy::unwrap_used, reason = "tests")]

#[path = "wasm_client/files.rs"]
mod files;
#[path = "wasm_client/limits.rs"]
mod limits;
#[path = "wasm_client/objects.rs"]
mod objects;
#[path = "wasm_client/support.rs"]
mod support;
#[path = "wasm_client/timer.rs"]
mod timer;

use std::rc::Rc;

use serde_json::{Value, json};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_test::wasm_bindgen_test as test;

use support::{
    Heard, fake_tunnel, from_js, get, settle, sleep, start, tunnel_state, with_snapshot,
};

#[test]
async fn a_client_starts_offline_takes_requests_and_comes_back_from_its_snapshot() {
    fake_tunnel("silent");
    let client = start(json!({})).unwrap();
    let ready = Heard::on(&client, "ready");
    let changes = Heard::on(&client, "change");
    sleep(20.0).await;
    assert_eq!(ready.all()[0]["name"], "Alice");
    assert!(
        !changes.all().is_empty(),
        "a new store announces its first save"
    );

    let club = settle(client.create_group("Club").into()).await.unwrap();
    assert_eq!(get(&club, "name"), "Club");
    assert_eq!(client.groups().size(), 1);

    let bytes = client.save().unwrap();
    client.close();
    assert!(client.save().is_err(), "a closed client saves nothing");

    let wrong = with_snapshot(&bytes, "nope").err().unwrap();
    assert_eq!(from_js(&wrong)["code"], "wrong_key");

    let again = with_snapshot(&bytes, "pass").unwrap();
    let ready = Heard::on(&again, "ready");
    sleep(20.0).await;
    assert_eq!(
        ready.all()[0]["name"],
        "Alice",
        "the snapshot keeps the profile"
    );
    let groups = again
        .groups()
        .values()
        .into_iter()
        .map(|g| get(&g.unwrap(), "name"));
    assert_eq!(groups.collect::<Vec<_>>(), [JsValue::from("Club")]);
    again.close();
}

#[test]
async fn a_refused_tunnel_reply_gives_a_clean_offline_event() {
    fake_tunnel("refuse");
    let client = start(json!({ "timeout": 300 })).unwrap();
    let offline = Heard::on(&client, "offline");
    let answer = settle(client.create_invite().into()).await.err().unwrap();
    assert_eq!(from_js(&answer)["code"], "timeout");
    assert_eq!(offline.all(), [Value::Null]);

    let state = tunnel_state();
    let hello: Value = serde_json::from_str(state["texts"][0].as_str().unwrap()).unwrap();
    assert_eq!(hello["port"], 5223);
    assert!(
        hello["host"].as_str().unwrap().ends_with(".simplex.im")
            || hello["host"].as_str().unwrap().contains("simplexonflux")
    );
    assert_eq!(state["types"][0], "arraybuffer");
    client.close();
}

#[test]
async fn a_handler_may_call_the_client_and_close_rejects_waiting_calls() {
    fake_tunnel("silent");
    let client = Rc::new(start(json!({})).unwrap());
    let c = Rc::clone(&client);
    let f = Closure::<dyn FnMut(JsValue)>::new(move |_| drop(c.set_name("Bob")));
    client.on(
        "ready",
        f.as_ref().unchecked_ref::<js_sys::Function>().clone(),
    );
    let raw = Heard::on(&client, "raw");
    sleep(20.0).await;
    assert_eq!(from_js(&client.user())["name"], "Bob");
    assert!(raw.all().iter().any(|e| e["t"] == "profile"));

    let invite = client.create_invite();
    client.close();
    let answer = settle(invite.into()).await.err().unwrap();
    assert_eq!(from_js(&answer)["code"], "closed");
    let late = settle(client.connect("simplex:/x").into())
        .await
        .err()
        .unwrap();
    assert_eq!(from_js(&late)["code"], "closed");
}

#[test]
fn bad_options_are_refused_with_a_code() {
    let e = simplex_client::wasm::Client::open(&support::to_js(&json!({ "name": "A" })))
        .err()
        .unwrap();
    assert_eq!(from_js(&e)["code"], "bad_option");
}
