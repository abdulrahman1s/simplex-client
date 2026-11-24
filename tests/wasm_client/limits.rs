//! The official client's limits as JS sees them: rejected promises with a `code` of their own.

use serde_json::json;
use simplex_client::types::Event;
use simplex_client::types::model::{
    ChatMessage, ChatRef, Delivery, Friend, MessageAuthor, MessageBody,
};
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::support::{Heard, call, fake_tunnel, from_js, settle, sleep, start};

async fn started() -> simplex_client::wasm::Client {
    fake_tunnel("silent");
    let client = start(json!({})).unwrap();
    sleep(20.0).await;
    client
}

#[test]
async fn a_reaction_outside_the_official_set_rejects_with_its_code() {
    let client = started().await;
    let heard = Heard::on(&client, "message");
    let bob = Friend {
        id: 4,
        name: "bob".into(),
        ready: true,
        ..Friend::default()
    };
    client.inject(Event::Chats {
        friends: vec![bob],
        groups: Vec::new(),
        requests: Vec::new(),
        chats: Vec::new(),
    });
    client.inject(Event::Messages(vec![ChatMessage {
        id: 9,
        chat: ChatRef::Direct(4),
        author: MessageAuthor::Friend,
        body: MessageBody::Text("ping".into()),
        at: 0,
        delivery: Delivery::Received,
        unread: true,
        reply: None,
        reactions: Vec::new(),
        edited: false,
        mentions: Vec::new(),
    }]));
    let message = heard.objects().pop().unwrap();
    let refused = settle(call(&message, "react", &["🤡".into()])).await;
    assert_eq!(
        from_js(&refused.unwrap_err())["code"],
        "reaction_not_allowed"
    );
    client.close();
}

#[test]
async fn a_text_too_long_and_a_bad_group_name_reject_with_their_codes() {
    let client = started().await;
    let club = settle(client.create_group("Club").into()).await.unwrap();
    let long = "x".repeat(simplex_client::limits::MAX_ENCODED_MSG_LENGTH + 1);
    let refused = settle(call(&club, "send", &[long.into()])).await;
    assert_eq!(from_js(&refused.unwrap_err())["code"], "text_too_long");
    settle(call(&club, "send", &["hi".into()])).await.unwrap();
    let bad = settle(client.create_group("#club").into()).await;
    assert_eq!(from_js(&bad.unwrap_err())["code"], "invalid_name");
    client.close();
}

#[test]
fn a_new_user_with_a_name_upstream_would_change_is_refused() {
    fake_tunnel("silent");
    let refused = start(json!({ "name": "J.Doe" })).err().unwrap();
    assert_eq!(from_js(&refused)["code"], "invalid_name");
}
