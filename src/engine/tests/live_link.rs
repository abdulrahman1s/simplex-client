//! A link with its preview against the official CLI 7.1.4, both ways; run as `live_cli` says.

use serde_json::{Value, json};

use crate::types::Request;
use crate::types::model::{ChatRef, LinkInfo, MessageAuthor};

use super::Dir;
use super::live_cli::{Native, cli_event, cli_send, connect, engine_msg, until, var};
use crate::agent::tests::cli::Cli;

#[test]
#[ignore = "needs a local smp-server, xftp-server and CLI; see live_cli"]
fn a_links_preview_goes_both_ways() {
    let dir = Dir::new();
    let mut n = Native::start(&dir);
    let mut c = Cli::open(var("SIMPLEX_TEST_CLI").parse().unwrap());
    let (cid, bob) = connect(&mut n, &mut c);
    n.req(Request::SendLink {
        chat: ChatRef::Direct(bob),
        text: "see https://simplex.chat".into(),
        preview: LinkInfo {
            uri: "https://simplex.chat".into(),
            title: "SimpleX".into(),
            description: "private".into(),
            image: None,
        },
        reply_to: None,
    });
    until(&mut n, &mut c, "the CLI gets the link", |_, c| {
        cli_event(c, "newChatItems", |e| {
            e.pointer("/chatItems/0/chatItem/content/msgContent/preview/uri")
                .and_then(Value::as_str)
                == Some("https://simplex.chat")
        })
    });
    let preview = json!({"uri": "https://example.org", "title": "Example", "description": "", "image": "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg=="});
    cli_send(
        &mut c,
        cid,
        json!({"msgContent": {"type": "link", "text": "https://example.org", "preview": preview}}),
    );
    let got = until(&mut n, &mut c, "the engine gets the link", |n, _| {
        engine_msg(n, |m| m.author == MessageAuthor::Friend && m.link.is_some())
    });
    assert_eq!(got.link.unwrap().uri, "https://example.org");
}
