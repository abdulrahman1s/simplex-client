//! Live messages against the official CLI 7.1.4, both ways; run as `live_cli` says.

use serde_json::Value;

use crate::types::Request;
use crate::types::model::{ChatRef, MessageAuthor};

use super::Dir;
use super::live_cli::{Native, cli_event, connect, engine_msg, until, var};
use crate::agent::tests::cli::{Cli, kind};

#[test]
#[ignore = "needs a local smp-server, xftp-server and CLI; see live_cli"]
fn a_live_message_reaches_the_cli_and_the_clis_reaches_the_engine() {
    let dir = Dir::new();
    let mut n = Native::start(&dir);
    let mut c = Cli::open(var("SIMPLEX_TEST_CLI").parse().unwrap());
    let (cid, bob) = connect(&mut n, &mut c);
    let chat = ChatRef::Direct(bob);
    n.req(Request::SendLive {
        chat,
        text: "wri".into(),
    });
    let mine = engine_msg(&mut n, |m| m.author == MessageAuthor::Me && m.live).unwrap();
    until(&mut n, &mut c, "the CLI gets a live message", |_, c| {
        cli_event(c, "newChatItems", |e| {
            e.pointer("/chatItems/0/chatItem/meta/itemLive")
                .and_then(Value::as_bool)
                == Some(true)
        })
    });
    n.req(Request::UpdateLive {
        chat,
        item: mine.id,
        text: "written".into(),
        done: true,
    });
    until(&mut n, &mut c, "the CLI sees it finished", |_, c| {
        cli_event(c, "chatItemUpdated", |e| {
            e.pointer("/chatItem/chatItem/content/msgContent/text")
                .and_then(Value::as_str)
                == Some("written")
        })
    });

    let r = c.cmd(&format!(
        r#"/_send @{cid} live=on json [{{"msgContent": {{"type": "text", "text": "ty"}}}}]"#
    ));
    assert_eq!(kind(&r), "newChatItems", "{r}");
    until(&mut n, &mut c, "the engine gets a live message", |n, _| {
        engine_msg(n, |m| m.author == MessageAuthor::Friend && m.live)
    });
}
