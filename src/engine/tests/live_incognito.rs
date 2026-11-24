//! Incognito against the official CLI 7.1.4: it sees only the random name; run as `live_cli` says.

use serde_json::Value;

use crate::types::{Event, Request};

use super::Dir;
use super::live_cli::{Native, until, var};
use crate::agent::tests::cli::{Cli, kind};

#[test]
#[ignore = "needs a local smp-server, xftp-server and CLI; see live_cli"]
fn the_cli_knows_an_incognito_invitation_by_its_random_name_only() {
    let dir = Dir::new();
    let mut n = Native::start(&dir);
    let mut c = Cli::open(var("SIMPLEX_TEST_CLI").parse().unwrap());
    n.req(Request::CreateIncognitoInvite);
    let link = until(&mut n, &mut c, "the invitation", |n, _| {
        n.events.iter().find_map(|e| match e {
            Event::Invite(l) => Some(l.clone()),
            _ => None,
        })
    });
    let r = c.cmd(&format!("/connect {link}"));
    assert!(!kind(&r).contains("Error"), "{r}");
    let shown = until(&mut n, &mut c, "the CLI's new contact", |_, c| {
        c.take(|e| {
            (kind(e) == "contactConnected").then(|| {
                e.pointer("/contact/profile/displayName")?
                    .as_str()
                    .map(str::to_owned)
            })?
        })
    });
    let ours =
        n.e.state
            .friends
            .values()
            .find_map(|f| f.incognito.clone())
            .unwrap();
    assert_eq!(shown, ours);
    assert_ne!(shown, "alice");
    let _ = Value::Null;
}
