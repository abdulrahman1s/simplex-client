//! Removing a friend right after writing to them, against the official CLI 7.1.4: both the message and the deletion arrive; run as `live_cli` says.

use crate::types::Request;
use crate::types::model::ChatRef;

use super::Dir;
use super::live_cli::{Native, cli_event, connect, until, var};
use crate::agent::tests::cli::{Cli, received_text};

#[test]
#[ignore = "needs a local smp-server, xftp-server and CLI; see live_cli"]
fn a_message_sent_just_before_removing_a_friend_arrives_and_so_does_the_removal() {
    let dir = Dir::new();
    let mut n = Native::start(&dir);
    let mut c = Cli::open(var("SIMPLEX_TEST_CLI").parse().unwrap());
    let (cid, bob) = connect(&mut n, &mut c);
    n.req(Request::Send {
        chat: ChatRef::Direct(bob),
        text: "goodbye".into(),
        reply_to: None,
        mentions: Vec::new(),
    });
    n.req(Request::RemoveFriend(bob));
    assert!(!n.e.state.friends.contains_key(&bob));
    until(&mut n, &mut c, "the CLI gets the last message", |_, c| {
        c.take(|e| received_text(e, cid).filter(|t| t == "goodbye"))
    });
    until(&mut n, &mut c, "the CLI hears it was removed", |_, c| {
        cli_event(c, "contactDeletedByContact", |e| {
            e.pointer("/contact/contactId")
                .and_then(serde_json::Value::as_i64)
                == Some(cid)
        })
    });
}
