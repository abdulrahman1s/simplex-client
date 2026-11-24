//! Chat preferences against the official CLI 7.1.4 (Profiles.hs `testSetContactPrefs`); run as `live_cli` says.

use serde_json::Value;

use crate::types::Request;
use crate::types::model::{Allow, ChatFeature, ChatPrefs};

use super::Dir;
use super::live_cli::{Native, cli_event, connect, until, var};
use crate::agent::tests::cli::{Cli, kind};

#[test]
#[ignore = "needs a local smp-server, xftp-server and CLI; see live_cli"]
fn an_override_reaches_the_cli_and_the_clis_reaches_the_engine() {
    let dir = Dir::new();
    let mut n = Native::start(&dir);
    let mut c = Cli::open(var("SIMPLEX_TEST_CLI").parse().unwrap());
    let (cid, bob) = connect(&mut n, &mut c);

    n.req(Request::SetPrefs {
        friend: Some(bob),
        prefs: ChatPrefs {
            calls: Some(Allow::No),
            ..ChatPrefs::default()
        },
    });
    until(&mut n, &mut c, "the CLI sees calls off", |_, c| {
        cli_event(c, "contactUpdated", |e| {
            e.pointer("/toContact/profile/preferences/calls/allow")
                .and_then(Value::as_str)
                == Some("no")
        })
    });

    let r = c.cmd(&format!(
        r#"/_set prefs @{cid} {{"reactions": {{"allow": "no"}}}}"#
    ));
    assert_eq!(kind(&r), "contactPrefsUpdated", "{r}");
    until(&mut n, &mut c, "the engine sees reactions off", |n, _| {
        let f = n.e.state.friends[&bob].clone();
        (!n.e.enabled(&f, ChatFeature::Reactions).for_contact).then_some(())
    });
}
