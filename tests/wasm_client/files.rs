//! Files through the JS objects: `sendFile` queues the bytes, `download` waits for them, `fileProgress` follows both.

use serde_json::json;
use simplex_client::types::model::{
    ChatMessage, ChatRef, Delivery, FileState, Friend, Media, MessageAuthor, MessageBody,
};
use simplex_client::types::{Event, FileKind, Request};
use wasm_bindgen::JsValue;
use wasm_bindgen_test::wasm_bindgen_test as test;

use super::support::{Heard, call, fake_tunnel, from_js, get, settle, sleep, start};

fn photo(state: FileState) -> ChatMessage {
    let media = Media {
        name: "cat.png".into(),
        size: 1000,
        preview: None,
        duration: None,
        caption: String::new(),
        local: None,
        state,
    };
    ChatMessage {
        id: 9,
        chat: ChatRef::Direct(4),
        author: MessageAuthor::Friend,
        body: MessageBody::File(media),
        at: 0,
        delivery: Delivery::Received,
        unread: true,
        reply: None,
        reactions: Vec::new(),
        edited: false,
        mentions: Vec::new(),
    }
}

async fn with_bob() -> simplex_client::wasm::Client {
    fake_tunnel("silent");
    let client = start(json!({})).unwrap();
    sleep(20.0).await;
    client.tap(true);
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
    client
}

#[test]
async fn send_file_hands_the_engine_a_memory_path_and_rejects_with_its_refusal() {
    let client = with_bob().await;
    let friend = client.friends().get(&JsValue::from(4));
    let bytes = js_sys::Uint8Array::from(&[1u8, 2, 3][..]);
    let sent = call(
        &friend,
        "sendFile",
        &[bytes.into(), "cat.png".into(), "image".into()],
    );
    let err = settle(sent).await.unwrap_err();
    assert_eq!(
        from_js(&err)["code"],
        "request_failed",
        "the engine has no friend 4"
    );
    let asked = client.tap(false).into_iter().find_map(|r| match r {
        Request::SendFile { path, kind, .. } => Some((path, kind)),
        _ => None,
    });
    let (path, kind) = asked.unwrap();
    assert!(
        path.starts_with("/mem") && path.ends_with("cat.png"),
        "{path:?}"
    );
    assert_eq!(kind, FileKind::Image);
    client.close();
}

#[test]
async fn download_asks_for_the_file_and_progress_is_heard() {
    let client = with_bob().await;
    let heard = Heard::on(&client, "message");
    let progress = Heard::on(&client, "fileProgress");
    client.inject(Event::Messages(vec![photo(FileState::Offered)]));
    let message = heard.objects().pop().unwrap();
    let refused = settle(call(&message, "download", &[])).await.unwrap_err();
    assert_eq!(
        from_js(&refused)["code"],
        "request_failed",
        "the engine has no item 9"
    );
    let asked = Request::ReceiveFile {
        chat: ChatRef::Direct(4),
        item: 9,
    };
    assert!(client.tap(true).contains(&asked));

    client.inject(Event::Updated(vec![photo(FileState::Receiving {
        percent: 40,
    })]));
    let p = progress.all();
    assert_eq!(
        p,
        [json!({ "friend": 4, "id": 9, "percent": 40, "sending": false })]
    );
    assert_eq!(get(&message, "id"), 9);
    client.close();
}
