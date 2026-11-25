//! A forwarded message against the official CLI 7.1.4; run as `live_cli` says.

use serde_json::Value;

use crate::types::Request;
use crate::types::model::{ChatRef, MessageAuthor};

use super::Dir;
use super::live_cli::{Native, cli_event, connect, engine_msg, until, var};
use crate::agent::tests::cli::{Cli, send_text};

#[test]
#[ignore = "needs a local smp-server, xftp-server and CLI; see live_cli"]
fn the_cli_sees_a_forwarded_message_as_forwarded() {
    let dir = Dir::new();
    let mut n = Native::start(&dir);
    let mut c = Cli::open(var("SIMPLEX_TEST_CLI").parse().unwrap());
    let (cid, bob) = connect(&mut n, &mut c);
    send_text(&mut c, cid, "pass this on");
    let theirs = until(&mut n, &mut c, "bob's message", |n, _| {
        engine_msg(n, |m| {
            m.author == MessageAuthor::Friend && m.text() == "pass this on"
        })
    });
    n.req(Request::Forward {
        from: ChatRef::Direct(bob),
        item: theirs.id,
        to: ChatRef::Direct(bob),
    });
    until(&mut n, &mut c, "the CLI gets it forwarded", |_, c| {
        cli_event(c, "newChatItems", |e| {
            let it = e.pointer("/chatItems/0/chatItem");
            it.and_then(|i| i.pointer("/meta/itemForwarded"))
                .is_some_and(|f| !f.is_null())
                && it
                    .and_then(|i| i.pointer("/content/msgContent/text"))
                    .and_then(Value::as_str)
                    == Some("pass this on")
        })
    });
}
