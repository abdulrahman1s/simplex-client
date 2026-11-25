//! A friend's negotiated PQ state against the official CLI 7.1.4, which makes its connections with PQ; run as `live_cli` says.

use serde_json::Value;

use crate::types::Event;
use crate::types::model::ChatRef;

use super::Dir;
use super::live_cli::{Native, cli_send, connect, until, var};
use crate::agent::tests::cli::Cli;

#[test]
#[ignore = "needs a local smp-server, xftp-server and CLI; see live_cli"]
fn a_friends_pq_is_on_once_a_message_went_each_way_with_the_official_cli() {
    let dir = Dir::new();
    let mut n = Native::start(&dir);
    let mut c = Cli::open(var("SIMPLEX_TEST_CLI").parse().unwrap());
    let (cid, bob) = connect(&mut n, &mut c);
    n.req(crate::types::Request::Send {
        chat: ChatRef::Direct(bob),
        text: "hello".into(),
        reply_to: None,
        mentions: Vec::new(),
    });
    cli_send(
        &mut c,
        cid,
        serde_json::json!({"msgContent": {"type": "text", "text": "hi"}}),
    );
    until(&mut n, &mut c, "the friend is listed with PQ on", |n, _| {
        n.events.iter().rev().find_map(|e| match e {
            Event::Chats { friends, .. } => {
                friends.iter().find(|f| f.id == bob && f.pq).map(|_| ())
            }
            _ => None,
        })
    });
    let r = c.cmd(&format!("/_info @{cid}"));
    let stats = r.pointer("/connectionStats").unwrap_or(&Value::Null);
    eprintln!("CLI connection: {stats}");
    let contact = r.pointer("/contact/activeConn").unwrap_or(&Value::Null);
    assert_eq!(contact["pqSndEnabled"], true, "{r}");
    assert_eq!(contact["pqRcvEnabled"], true, "{r}");
}
