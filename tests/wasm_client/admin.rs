//! Group admin from JS: a listed member's role and removal, the group's profile, preferences and deletion, and moderation.

use serde_json::json;
use simplex_client::types::Request;
use simplex_client::types::model::{GroupPref, GroupPrefs, MemberRole};
use wasm_bindgen::JsValue;
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::support::{call, fake_tunnel, from_js, get, settle, sleep, start, to_js};

#[allow(clippy::cast_possible_truncation, reason = "ids are small counters")]
fn id(v: &JsValue) -> i64 {
    get(v, "id").as_f64().unwrap() as i64
}

#[test]
async fn admin_calls_queue_their_requests_and_bad_arguments_are_refused() {
    fake_tunnel("silent");
    let client = start(json!({})).unwrap();
    sleep(20.0).await;
    let group = settle(client.create_group("team").into()).await.unwrap();
    let g = id(&group);
    let listed: js_sys::Array = settle(call(&group, "members", &[])).await.unwrap().into();
    let member = listed.get(0);
    let m = id(&member);
    client.tap(true);

    settle(call(&member, "setRole", &["admin".into()]))
        .await
        .unwrap();
    let bad = settle(call(&member, "setRole", &["king".into()]))
        .await
        .err()
        .unwrap();
    assert_eq!(from_js(&bad)["code"], "bad_argument");
    settle(call(&member, "remove", &[true.into()]))
        .await
        .unwrap();
    settle(call(&group, "setProfile", &["crew".into(), JsValue::NULL]))
        .await
        .unwrap();
    let prefs = to_js(&json!({ "voice": { "on": false } }));
    settle(call(&group, "setPrefs", &[prefs])).await.unwrap();
    settle(call(&group, "delete", &[])).await.unwrap();

    let asked = client.tap(false);
    let voice_off = GroupPrefs {
        voice: Some(GroupPref {
            on: false,
            role: None,
        }),
        ..GroupPrefs::default()
    };
    for want in [
        Request::SetMemberRole {
            group: g,
            member: m,
            role: MemberRole::Admin,
        },
        Request::RemoveMember {
            group: g,
            member: m,
            messages: true,
        },
        Request::SetGroupProfile {
            group: g,
            name: "crew".into(),
            description: None,
        },
        Request::SetGroupPrefs {
            group: g,
            prefs: voice_off,
        },
        Request::DeleteGroup(g),
    ] {
        assert!(asked.contains(&want), "{want:?} in {asked:?}");
    }
    client.close();
}
