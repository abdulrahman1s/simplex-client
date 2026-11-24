//! Incognito: a friend who knows the user by a random name only (Commands.hs `APIAddContact`, `APIConnect`, `APIAcceptContact` incognito; ChatTests/Profiles.hs `testConnectIncognitoInvitationLink`).

use crate::types::Request;
use crate::types::model::ChatProfile;

use super::{Call, T, x_info};
use crate::engine::agent_port::AgentEvent;
use crate::engine::profile::received_profile;

/// The name in the last conn-info the engine gave the agent to allow with.
fn allowed_as(t: &T) -> String {
    let info = t.e.agent.calls.iter().rev().find_map(|c| match c {
        Call::Allow { conn_info, .. } => Some(conn_info.clone()),
        _ => None,
    });
    received_profile(&info.unwrap()).unwrap().display_name
}

#[test]
fn an_incognito_invitation_answers_by_a_random_name_that_stays_theirs() {
    let mut t = T::started();
    t.req(Request::CreateIncognitoInvite);
    let conn = t.e.state.inviting.unwrap();
    t.agent(AgentEvent::Invitation {
        conn,
        full: "simplex:/invitation#/?v=2-7&name=x".into(),
        short: None,
    });
    t.agent(AgentEvent::Conf {
        conn,
        conn_info: x_info("bob"),
    });
    let name = allowed_as(&t);
    assert_ne!(name, "alice");
    let bob = t.e.state.friend_of(&conn).unwrap();
    assert_eq!(
        t.e.state.friends[&bob].incognito.as_deref(),
        Some(name.as_str())
    );
    t.agent(AgentEvent::Con { conn });
    let sent = t.e.agent.sent().len();
    t.req(Request::SetProfile(ChatProfile {
        name: "alice2".into(),
        ..ChatProfile::default()
    }));
    assert_eq!(
        t.e.agent.sent().len(),
        sent,
        "the user's own profile never goes to them"
    );
}

#[test]
fn connecting_incognito_joins_with_a_random_name() {
    let mut t = T::started();
    t.req(Request::ConnectIncognito(
        "simplex:/invitation#/?v=2-7&smp=smp%3A%2F%2Fu2dS9sG8nMNURyZwqASV4yROM28Er0luVTx5X1CsMrU%3D%40smp4.simplex.im%2FabcdefghijklmnopqrstuvwxyzAB%23%2F%3Fv%3D1-3%26dh%3DMCowBQYDK2VuAyEAjiswwI3O_NlS8Fk3HJUW870EY2bAwmttMBsvRB9eV3o%253D&e2e=v%3D2-3%26x3dh%3DMEIwBQYDK2VvAzkAmKuSYeQ_m0SixPDS8Wq8VBaTS1cW-Lp0n0h4Diu-kUpR-qXx4SDJ32YGEFoGFGSbGPry5Ly7_5Y%3D%2CMEIwBQYDK2VvAzkA4XnaVNd5AeCqkd_kRrz6hNEbMhfbG_V5CfKm9Mh4lFFGa3tiKLJyB8a0JrWb4dO0NC5ALg0_SO4%3D".into(),
    ));
    let joined = t.e.agent.calls.iter().find_map(|c| match c {
        Call::Join { conn_info, .. } => Some(conn_info.clone()),
        _ => None,
    });
    let Some(info) = joined else {
        let failed = t.out();
        panic!("not joined: {failed:?}")
    };
    let name = received_profile(&info).unwrap().display_name;
    assert_ne!(name, "alice");
    let f = t.e.state.friends.values().next().unwrap();
    assert_eq!(f.incognito.as_deref(), Some(name.as_str()));
}

/// A friend met through an incognito invitation.
fn incognito_friend(t: &mut T) -> i64 {
    t.req(Request::CreateIncognitoInvite);
    let conn = t.e.state.inviting.unwrap();
    t.agent(AgentEvent::Invitation {
        conn,
        full: "simplex:/invitation#/?v=2-7&name=x".into(),
        short: None,
    });
    t.agent(AgentEvent::Conf {
        conn,
        conn_info: x_info("bob"),
    });
    t.agent(AgentEvent::Con { conn });
    t.e.state.friend_of(&conn).unwrap()
}

#[test]
fn a_friend_met_incognito_never_sees_the_user_in_a_group() {
    let mut t = T::started();
    let bob = incognito_friend(&mut t);
    let (g, _) = t.group(
        "team",
        &["carol"],
        crate::chat::group::GroupMemberRole::Owner,
    );
    let out = t.req(Request::AddMember {
        group: g,
        friend: bob,
    });
    assert!(
        out.iter()
            .any(|e| matches!(e, crate::types::Event::Failed { .. })),
        "CEContactIncognitoCantInvite"
    );
    let mut joined = t.e.group(g).unwrap();
    joined.invited_by_friend = Some(bob);
    joined.status = crate::engine::records::MemberStatus::Invited;
    t.e.save_group(joined).unwrap();
    let out = t.req(Request::JoinGroup(g));
    assert!(
        out.iter()
            .any(|e| matches!(e, crate::types::Event::Failed { .. }))
    );
    let out = t.req(Request::ConnectIncognito(
        super::groups::links::SHORT.into(),
    ));
    let why = out.iter().find_map(|e| match e {
        crate::types::Event::Failed { detail, .. } => Some(detail.clone()),
        _ => None,
    });
    assert_eq!(why.as_deref(), Some("a group is not joined incognito here"));
}

#[test]
fn a_friend_met_incognito_gets_the_default_preferences_not_the_users() {
    let mut t = T::started();
    t.req(Request::SetPrefs {
        friend: None,
        prefs: crate::types::model::ChatPrefs {
            voice: Some(crate::types::model::Allow::No),
            ..Default::default()
        },
    });
    let bob = incognito_friend(&mut t);
    let f = t.e.state.friends[&bob].clone();
    assert!(
        t.e.enabled(&f, crate::types::model::ChatFeature::Voice)
            .for_user
    );
}
