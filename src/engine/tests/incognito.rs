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
