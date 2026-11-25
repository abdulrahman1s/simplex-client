//! A direct chat with a group member against two official CLIs 7.1.4; run as `live_group` says.

use serde_json::{Value, json};

use crate::types::Request;
use crate::types::model::Membership;

use super::Dir;
use super::live_group::{Who, World, befriend, cli_friends, cli_joins, engine_group, group_id};
use crate::agent::tests::cli::kind;

#[test]
#[ignore = "needs a local smp-server and two CLIs; see live_group"]
fn a_member_the_engine_contacts_accepts_and_they_become_friends() {
    let dir = Dir::new();
    let mut w = World::start(&dir);
    let (alice_at_bob, _) = befriend(&mut w, Who::Bob);
    let carol_at_bob = cli_friends(&mut w);
    let r = w.bob.cmd(&format!(
        "/_group 1 {}",
        json!({"displayName": "talk", "fullName": "", "groupPreferences": {"directMessages": {"enable": "on"}}})
    ));
    let bg = group_id(&r).unwrap();
    w.bob.cmd(&format!("/_add #{bg} {alice_at_bob} member"));
    let g = engine_group(&mut w, "talk", Membership::Invited);
    w.n.req(Request::JoinGroup(g));
    engine_group(&mut w, "talk", Membership::Member);
    w.bob.cmd(&format!("/_add #{bg} {carol_at_bob} member"));
    cli_joins(&mut w, Who::Carol);
    let carol = w.until("the engine meets carol", |w| {
        let mut all = w.n.e.state.members_of(g);
        all.find(|m| m.profile.display_name == "carol" && m.conn.is_some() && m.status.current())
            .map(|m| m.id)
    });

    w.n.req(Request::ContactMember {
        group: g,
        member: carol,
        text: Some("hi carol, directly".into()),
    });
    let cid = w.until("carol is asked", |w| {
        w.carol.take(|e| {
            (kind(e) == "newMemberContactReceivedInv")
                .then(|| e.pointer("/contact/contactId")?.as_i64())?
        })
    });
    let r = w.carol.cmd(&format!("/_accept member contact @{cid}"));
    assert!(!kind(&r).contains("Error"), "{r}");
    w.until("the engine's friend carol is connected", |w| {
        let f = w.n.e.state.friends.values();
        f.filter(|f| f.name == "carol" && f.ready)
            .map(|f| f.id)
            .next()
    });
    let _ = Value::Null;
}
