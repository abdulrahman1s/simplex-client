//! Disappearing messages against the official CLI 7.1.4 (Profiles.hs `testEnableTimedMessagesContact`); run as `live_cli` says.

use serde_json::Value;

use crate::types::Request;
use crate::types::model::{ChatRef, MessageAuthor, MessageBody};

use super::Dir;
use super::live_cli::{Native, cli_event, connect, engine_msg, until, var};
use crate::agent::tests::cli::{Cli, send_text};

/// The ttl of the item the CLI received from `cid` in `newChatItems`.
fn received_ttl(e: &Value, cid: i64) -> Option<i64> {
    e.get("chatItems")?.as_array()?.iter().find_map(|ci| {
        let from = ci.pointer("/chatInfo/contact/contactId")?.as_i64()?;
        let dir = ci.pointer("/chatItem/chatDir/type")?.as_str()?;
        let ttl = ci.pointer("/chatItem/meta/itemTimed/ttl")?.as_i64()?;
        (from == cid && dir == "directRcv").then_some(ttl)
    })
}

#[test]
#[ignore = "needs a local smp-server, xftp-server and CLI; see live_cli"]
fn the_clis_timer_is_adopted_and_both_sides_messages_disappear() {
    let dir = Dir::new();
    let mut n = Native::start(&dir);
    let mut c = Cli::open(var("SIMPLEX_TEST_CLI").parse().unwrap());
    let (cid, bob) = connect(&mut n, &mut c);
    let chat = ChatRef::Direct(bob);

    let r = c.cmd(&format!(
        r#"/_set prefs @{cid} {{"timedMessages": {{"allow": "yes", "ttl": 3}}}}"#
    ));
    assert_eq!(
        crate::agent::tests::cli::kind(&r),
        "contactPrefsUpdated",
        "{r}"
    );
    until(&mut n, &mut c, "the engine adopts the timer", |n, _| {
        let f = &n.e.state.friends[&bob];
        (f.my_timer.as_ref()?.ttl == Some(3)).then_some(())
    });

    n.req(Request::Send {
        chat,
        text: "gone soon".into(),
        reply_to: None,
        mentions: Vec::new(),
    });
    until(&mut n, &mut c, "the CLI gets a 3-second message", |_, c| {
        cli_event(c, "newChatItems", |e| received_ttl(e, cid) == Some(3))
    });
    until(&mut n, &mut c, "the engine's own goes", |n, _| {
        engine_msg(n, |m| {
            m.author == MessageAuthor::Me && m.body == MessageBody::Deleted
        })
    });

    send_text(&mut c, cid, "mine too");
    let theirs = until(&mut n, &mut c, "a timed message from the CLI", |n, _| {
        engine_msg(n, |m| {
            m.author == MessageAuthor::Friend && m.timed.is_some()
        })
    });
    assert_eq!(theirs.timed.unwrap().ttl, 3);
    n.req(Request::MarkRead(chat));
    until(&mut n, &mut c, "read, it goes too", |n, _| {
        engine_msg(n, |m| m.id == theirs.id && m.body == MessageBody::Deleted)
    });

    n.req(Request::SetTimer { chat, ttl: None });
    until(&mut n, &mut c, "the CLI sees it turned off", |_, c| {
        cli_event(c, "contactUpdated", |e| {
            e.pointer("/toContact/profile/preferences/timedMessages/allow")
                .and_then(Value::as_str)
                == Some("no")
        })
    });
}
