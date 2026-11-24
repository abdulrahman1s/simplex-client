//! Settings from JS: `setTimer` and `setPrefs` on a friend or the client, and a message's `ttl` and `expiresAt`.

use serde_json::json;
use simplex_client::types::model::{
    Allow, ChatMessage, ChatPrefs, ChatRef, Delivery, Friend, MessageAuthor, MessageBody, Timed,
};
use simplex_client::types::{Event, Request};
use wasm_bindgen::JsValue;
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::support::{Heard, call, fake_tunnel, from_js, get, settle, sleep, start};

fn chats(friends: Vec<Friend>) -> Event {
    Event::Chats {
        friends,
        groups: Vec::new(),
        requests: Vec::new(),
        chats: Vec::new(),
    }
}

#[test]
async fn set_timer_takes_whole_seconds_or_null_and_a_message_shows_its_timer() {
    fake_tunnel("silent");
    let client = start(json!({})).unwrap();
    sleep(20.0).await;
    client.tap(true);
    let added = Heard::on(&client, "friendAdd");
    let bob = Friend {
        id: 4,
        name: "bob".into(),
        ready: true,
        timer: Some(30),
        ..Friend::default()
    };
    client.inject(chats(vec![bob]));
    let friend = added.objects().pop().unwrap();
    assert_eq!(get(&friend, "timer"), 30);

    settle(call(&friend, "setTimer", &[3600.into()]))
        .await
        .unwrap();
    settle(call(&friend, "setTimer", &[JsValue::NULL]))
        .await
        .unwrap();
    let bad = settle(call(&friend, "setTimer", &[1.5.into()]))
        .await
        .err()
        .unwrap();
    assert_eq!(from_js(&bad)["code"], "bad_argument");
    let asked = client.tap(false);
    let timer = |ttl| Request::SetTimer {
        chat: ChatRef::Direct(4),
        ttl,
    };
    assert!(asked.contains(&timer(Some(3600))) && asked.contains(&timer(None)));
    assert_eq!(
        asked
            .iter()
            .filter(|r| matches!(r, Request::SetTimer { .. }))
            .count(),
        2
    );

    let heard = Heard::on(&client, "message");
    client.inject(Event::Messages(vec![ChatMessage {
        id: 9,
        chat: ChatRef::Direct(4),
        author: MessageAuthor::Friend,
        body: MessageBody::Text("soon gone".into()),
        at: 0,
        delivery: Delivery::Received,
        unread: true,
        reply: None,
        reactions: Vec::new(),
        edited: false,
        mentions: Vec::new(),
        link: None,
        forwarded: false,
        timed: Some(Timed {
            ttl: 30,
            delete_at: None,
        }),
    }]));
    let msg = heard.objects().pop().unwrap();
    assert_eq!(get(&msg, "ttl"), 30);
    assert!(
        get(&msg, "expiresAt").is_undefined(),
        "unread: not counting yet"
    );
    client.close();
}

#[test]
async fn set_prefs_takes_each_feature_as_always_yes_or_no() {
    fake_tunnel("silent");
    let client = start(json!({})).unwrap();
    sleep(20.0).await;
    client.tap(true);
    let added = Heard::on(&client, "friendAdd");
    let bob = Friend {
        id: 4,
        name: "bob".into(),
        ready: true,
        ..Friend::default()
    };
    client.inject(chats(vec![bob]));
    let friend = added.objects().pop().unwrap();
    let prefs = super::support::to_js(&json!({ "voice": "no" }));
    settle(call(&friend, "setPrefs", &[prefs])).await.unwrap();
    let mine = super::support::to_js(&json!({ "calls": "always" }));
    settle(client.set_prefs(mine).into()).await.unwrap();
    let bad = super::support::to_js(&json!({ "voice": "maybe" }));
    let refused = settle(call(&friend, "setPrefs", &[bad]))
        .await
        .err()
        .unwrap();
    assert_eq!(from_js(&refused)["code"], "bad_argument");
    let asked = client.tap(false);
    let voice_no = ChatPrefs {
        voice: Some(Allow::No),
        ..ChatPrefs::default()
    };
    let calls_always = ChatPrefs {
        calls: Some(Allow::Always),
        ..ChatPrefs::default()
    };
    assert!(asked.contains(&Request::SetPrefs {
        friend: Some(4),
        prefs: voice_no
    }));
    assert!(asked.contains(&Request::SetPrefs {
        friend: None,
        prefs: calls_always
    }));
    client.close();
}
