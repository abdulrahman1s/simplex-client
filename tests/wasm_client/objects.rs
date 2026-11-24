//! The objects handlers get act on the client: requests, friends, messages, and the caches.

use serde_json::json;
use simplex_client::types::model::{
    ChatMessage, ChatRef, Delivery, Friend, FriendRequest, MessageAuthor, MessageBody,
};
use simplex_client::types::{Event, Request};
use wasm_bindgen::JsValue;
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::support::{Heard, call, fake_tunnel, get, settle, sleep, start};

fn chats(friends: Vec<Friend>, requests: Vec<FriendRequest>) -> Event {
    Event::Chats {
        friends,
        groups: Vec::new(),
        requests,
        chats: Vec::new(),
    }
}

fn bob(ready: bool) -> Friend {
    Friend {
        id: 4,
        name: "bob".into(),
        ready,
        ..Friend::default()
    }
}

/// A started client whose first chat list has been heard, with its requests watched.
async fn started() -> simplex_client::wasm::Client {
    fake_tunnel("silent");
    let client = start(json!({})).unwrap();
    sleep(20.0).await;
    client.tap(true);
    client
}

#[test]
async fn a_request_objects_accept_sends_accept_with_its_id() {
    let client = started().await;
    let heard = Heard::on(&client, "request");
    let carol = FriendRequest {
        id: 7,
        name: "carol".into(),
    };
    client.inject(chats(Vec::new(), vec![carol]));
    let request = heard.objects().pop().unwrap();
    assert_eq!(get(&request, "name"), "carol");
    settle(call(&request, "accept", &[])).await.unwrap();
    assert!(client.tap(false).contains(&Request::Accept(7)));
    client.close();
}

#[test]
async fn a_message_objects_reply_quotes_it_in_its_chat() {
    let client = started().await;
    let heard = Heard::on(&client, "message");
    client.inject(chats(vec![bob(true)], Vec::new()));
    let msg = ChatMessage {
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
        timed: None,
        link: None,
        forwarded: false,
        live: false,
    };
    client.inject(Event::Messages(vec![msg]));
    let message = heard.objects().pop().unwrap();
    assert_eq!(get(&message, "text"), "ping");
    assert_eq!(get(&get(&message, "author"), "name"), "bob");
    assert_eq!(get(&get(&message, "chat"), "id"), 4);
    settle(call(&message, "reply", &["pong".into()]))
        .await
        .unwrap();
    let sent = Request::Send {
        chat: ChatRef::Direct(4),
        text: "pong".into(),
        reply_to: Some(9),
        mentions: Vec::new(),
    };
    assert!(client.tap(false).contains(&sent));
    client.close();
}

#[test]
async fn a_new_friend_is_written_to_through_its_object() {
    let client = started().await;
    let added = Heard::on(&client, "friendAdd");
    client.inject(chats(vec![bob(false)], Vec::new()));
    assert!(
        added.objects().is_empty(),
        "a friend still connecting is not news"
    );
    client.inject(chats(vec![bob(true)], Vec::new()));
    let friend = added.objects().pop().unwrap();
    settle(call(&friend, "send", &["welcome".into()]))
        .await
        .unwrap();
    let sent = Request::Send {
        chat: ChatRef::Direct(4),
        text: "welcome".into(),
        reply_to: None,
        mentions: Vec::new(),
    };
    assert!(client.tap(false).contains(&sent));
    client.close();
}

#[test]
async fn the_friends_cache_follows_the_chat_list() {
    let client = started().await;
    let removed = Heard::on(&client, "friendRemove");
    client.inject(chats(vec![bob(true)], Vec::new()));
    let cached = client.friends().get(&JsValue::from(4));
    assert_eq!(get(&cached, "name"), "bob");
    client.inject(chats(Vec::new(), Vec::new()));
    assert_eq!(client.friends().size(), 0);
    assert_eq!(removed.all()[0]["name"], "bob");
    client.close();
}
