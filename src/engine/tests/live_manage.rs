//! An owner's group admin against two official CLIs 7.1.4: roles, blocks, the profile and preferences, moderation, removal and deletion; run as `live_group` says.

use serde_json::Value;

use crate::types::Request;
use crate::types::model::{
    ChatRef, GroupPref, GroupPrefs, MemberRole, Membership, MessageAuthor, MessageBody,
};

use super::Dir;
use super::live_cli::engine_msg;
use super::live_group::{Who, World, befriend, cli_group_send, cli_joins, engine_group};
use crate::agent::tests::cli::kind;

fn member_of(w: &World, group: i64, name: &str) -> i64 {
    let mut all = w.n.e.state.members_of(group);
    all.find(|m| m.profile.display_name == name).unwrap().id
}

/// Waits for an event of `k` at `who` that `f` accepts.
fn cli_sees(w: &mut World, who: Who, k: &str, f: impl Fn(&Value) -> bool) {
    w.until(&format!("{k} at {}", who.name()), |w| {
        w.cli(who).take(|e| (kind(e) == k && f(e)).then_some(()))
    });
}

#[test]
#[ignore = "needs a local smp-server and two CLIs; see live_group"]
fn an_owners_changes_reach_both_official_clis() {
    let dir = Dir::new();
    let mut w = World::start(&dir);
    let (_, bob) = befriend(&mut w, Who::Bob);
    let (_, carol) = befriend(&mut w, Who::Carol);
    w.n.req(Request::NewGroup("admins".into()));
    let g = engine_group(&mut w, "admins", Membership::Member);
    w.n.req(Request::AddMember {
        group: g,
        friend: bob,
    });
    cli_joins(&mut w, Who::Bob);
    w.n.req(Request::AddMember {
        group: g,
        friend: carol,
    });
    let cg = cli_joins(&mut w, Who::Carol);
    let (b, c) = (member_of(&w, g, "bob"), member_of(&w, g, "carol"));

    w.n.req(Request::SetMemberRole {
        group: g,
        member: c,
        role: MemberRole::Moderator,
    });
    cli_sees(&mut w, Who::Bob, "memberRole", |e| {
        e.pointer("/toRole").and_then(Value::as_str) == Some("moderator")
    });

    w.n.req(Request::BlockMemberForAll {
        group: g,
        member: b,
        on: true,
    });
    cli_sees(&mut w, Who::Carol, "memberBlockedForAll", |_| true);

    w.n.req(Request::SetGroupProfile {
        group: g,
        name: "crew".into(),
        description: Some("Welcome!".into()),
    });
    cli_sees(&mut w, Who::Carol, "groupUpdated", |e| {
        e.pointer("/toGroup/groupProfile/displayName")
            .and_then(Value::as_str)
            == Some("crew")
    });

    let voice_off = GroupPrefs {
        voice: Some(GroupPref {
            on: false,
            role: None,
        }),
        ..GroupPrefs::default()
    };
    w.n.req(Request::SetGroupPrefs {
        group: g,
        prefs: voice_off,
    });
    cli_sees(&mut w, Who::Carol, "groupUpdated", |e| {
        e.pointer("/toGroup/fullGroupPreferences/voice/enable")
            .and_then(Value::as_str)
            == Some("off")
    });

    cli_group_send(&mut w, Who::Carol, cg, "spam");
    let spam = w.until("carol's message", |w| {
        engine_msg(&mut w.n, |m| {
            m.author == MessageAuthor::Member("carol".into())
                && m.body == MessageBody::Text("spam".into())
        })
    });
    w.n.req(Request::Moderate {
        group: g,
        item: spam.id,
    });
    cli_sees(&mut w, Who::Carol, "chatItemsDeleted", |_| true);
    assert!(
        w.n.e
            .item(ChatRef::Group(g), spam.id)
            .unwrap()
            .is_none_or(|i| i.body == MessageBody::Deleted)
    );

    w.n.req(Request::RemoveMember {
        group: g,
        member: b,
        messages: false,
    });
    cli_sees(&mut w, Who::Bob, "deletedMemberUser", |_| true);
    cli_sees(&mut w, Who::Carol, "deletedMember", |_| true);

    w.n.req(Request::DeleteGroup(g));
    cli_sees(&mut w, Who::Carol, "groupDeleted", |_| true);
    assert!(!w.n.e.state.groups.contains_key(&g));
}
