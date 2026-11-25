//! Application data from JS: `sendData` queues `SendData`, data that is not an object is refused, and `dataTags` is a start option.

use serde_json::json;
use simplex_client::types::Request;
use simplex_client::types::model::ChatRef;
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::support::{call, fake_tunnel, from_js, get, settle, sleep, start, to_js, untagged};

#[test]
async fn send_data_queues_its_request_and_refuses_data_that_is_not_an_object() {
    fake_tunnel("silent");
    let client = start(json!({ "dataTags": ["app.poll"] })).unwrap();
    sleep(20.0).await;
    let group = settle(client.create_group("team").into()).await.unwrap();
    #[allow(clippy::cast_possible_truncation, reason = "ids are small counters")]
    let chat = ChatRef::Group(get(&group, "id").as_f64().unwrap() as i64);
    client.tap(true);
    let data = to_js(&json!({ "q": "lunch?" }));
    settle(call(
        &group,
        "sendData",
        &["app.poll".into(), data, "a poll".into()],
    ))
    .await
    .unwrap();
    let bad = call(
        &group,
        "sendData",
        &["app.poll".into(), 3.into(), "x".into()],
    );
    let bad = settle(bad).await.err().unwrap();
    assert_eq!(from_js(&bad)["code"], "bad_argument");
    let refused = call(
        &group,
        "sendData",
        &["text".into(), to_js(&json!({})), "x".into()],
    );
    let refused = settle(refused).await.err().unwrap();
    assert_eq!(from_js(&refused)["code"], "bad_argument");
    let asked = untagged(client.tap(false));
    let mut json = serde_json::Map::new();
    json.insert("q".into(), "lunch?".into());
    let want = Request::SendData {
        chat,
        tag: "app.poll".into(),
        json,
        fallback_text: "a poll".into(),
    };
    assert!(asked.contains(&want), "{want:?} in {asked:?}");
    client.close();
}

#[test]
async fn data_tags_that_are_not_strings_are_a_bad_option() {
    fake_tunnel("silent");
    let Err(e) = start(json!({ "dataTags": [1] })) else {
        panic!("started")
    };
    assert_eq!(from_js(&e)["code"], "bad_option");
}
