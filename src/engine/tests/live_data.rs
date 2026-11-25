//! Application data against the official CLI 7.1.4, both ways: it keeps and shows the fallback text, and what it sends under a registered tag is heard as data; run as `live_cli` says.

use serde_json::{Value, json};

use crate::types::model::ChatRef;
use crate::types::{Event, Request};

use super::Dir;
use super::data::obj;
use super::live_cli::{Native, cli_event, cli_send, connect, until, var};
use crate::agent::tests::cli::Cli;

#[test]
#[ignore = "needs a local smp-server, xftp-server and CLI; see live_cli"]
fn app_data_shows_as_its_fallback_in_the_cli_and_comes_back_as_data() {
    let dir = Dir::new();
    let mut n = Native::start_with_tags(&dir, &["app.poll"]);
    let mut c = Cli::open(var("SIMPLEX_TEST_CLI").parse().unwrap());
    let (cid, bob) = connect(&mut n, &mut c);
    n.req(Request::SendData {
        chat: ChatRef::Direct(bob),
        tag: "app.poll".into(),
        json: obj(json!({"q": "lunch?"})),
        fallback_text: "a poll: lunch?".into(),
    });
    let item = until(&mut n, &mut c, "the CLI keeps the fallback", |_, c| {
        cli_event(c, "newChatItems", |e| {
            e.pointer("/chatItems/0/chatItem/content/msgContent/type")
                .and_then(Value::as_str)
                == Some("app.poll")
        })
    });
    let content = item.pointer("/chatItems/0/chatItem/content").unwrap();
    assert_eq!(content["type"], "rcvMsgContent", "{content}");
    assert_eq!(content["msgContent"]["text"], "a poll: lunch?");
    assert_eq!(content["msgContent"]["q"], "lunch?");
    let shown = item.pointer("/chatItems/0/chatItem/meta/itemText");
    assert_eq!(shown.and_then(Value::as_str), Some("a poll: lunch?"));

    let msgs = n.events.len();
    cli_send(
        &mut c,
        cid,
        json!({"msgContent": {"type": "app.poll", "text": "a poll", "q": "dinner?"}}),
    );
    let data = until(&mut n, &mut c, "the engine hears the data", |n, _| {
        n.events.iter().find_map(|e| match e {
            Event::Data {
                chat, tag, json, ..
            } => Some((*chat, tag.clone(), json.clone())),
            _ => None,
        })
    });
    assert_eq!(
        data,
        (
            ChatRef::Direct(bob),
            "app.poll".into(),
            obj(json!({"q": "dinner?"}))
        )
    );
    let kept = n.events[msgs..].iter().any(|e| match e {
        Event::Messages(m) => m.iter().any(|m| m.text() == "a poll"),
        _ => false,
    });
    assert!(!kept, "data was kept as a message");
}
