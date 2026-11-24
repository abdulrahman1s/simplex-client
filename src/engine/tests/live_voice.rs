//! A voice message against the official CLI 7.1.4; run as `live_cli` says.

use serde_json::Value;

use crate::types::model::ChatRef;
use crate::types::{FileKind, Request};

use super::Dir;
use super::live_cli::{Native, cli_event, connect, until, var};
use crate::agent::tests::cli::Cli;

#[test]
#[ignore = "needs a local smp-server, xftp-server and CLI; see live_cli"]
fn the_cli_gets_a_voice_message_with_its_length() {
    let dir = Dir::new();
    let mut n = Native::start(&dir);
    let mut c = Cli::open(var("SIMPLEX_TEST_CLI").parse().unwrap());
    let (cid, bob) = connect(&mut n, &mut c);
    let path = dir.0.join("files").join("note.m4a");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, vec![7u8; 4_000]).unwrap();
    n.req(Request::SendFile {
        chat: ChatRef::Direct(bob),
        path,
        kind: FileKind::Voice,
        preview: None,
        duration: Some(9),
        text: String::new(),
    });
    until(
        &mut n,
        &mut c,
        "the CLI gets a 9-second voice message",
        |_, c| {
            cli_event(c, "newChatItems", |e| {
                let content = e.pointer("/chatItems/0/chatItem/content/msgContent");
                let from = e.pointer("/chatItems/0/chatInfo/contact/contactId");
                from.and_then(Value::as_i64) == Some(cid)
                    && content.and_then(|c| c.get("type")).and_then(Value::as_str) == Some("voice")
                    && content
                        .and_then(|c| c.get("duration"))
                        .and_then(Value::as_i64)
                        == Some(9)
            })
        },
    );
}
