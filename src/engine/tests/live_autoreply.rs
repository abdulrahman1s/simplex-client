//! The address's auto-accept and auto-reply against the official CLI 7.1.4; run as `live_cli` says.

use crate::types::{Event, Request};

use super::Dir;
use super::live_cli::{Native, until, var};
use crate::agent::tests::cli::{Cli, connected, kind, received_text};

#[test]
#[ignore = "needs a local smp-server, xftp-server and CLI; see live_cli"]
fn the_cli_is_accepted_at_once_and_gets_the_auto_reply() {
    let dir = Dir::new();
    let mut n = Native::start(&dir);
    let mut c = Cli::open(var("SIMPLEX_TEST_CLI").parse().unwrap());
    n.req(Request::CreateAddress);
    let link = until(&mut n, &mut c, "the address", |n, _| {
        n.events.iter().find_map(|e| match e {
            Event::Address(Some(l)) => Some(l.clone()),
            _ => None,
        })
    });
    n.req(Request::SetAddressSettings {
        auto_accept: true,
        auto_reply: Some("Hello, I answer at once.".into()),
    });
    let r = c.cmd(&format!("/connect {link}"));
    assert!(!kind(&r).contains("Error"), "{r}");
    let cid = until(
        &mut n,
        &mut c,
        "bob is connected without anyone accepting",
        |_, c| c.take(|e| connected(e, "alice")),
    );
    until(&mut n, &mut c, "bob gets the auto-reply", |_, c| {
        c.take(|e| received_text(e, cid).filter(|t| t == "Hello, I answer at once."))
    });
}
