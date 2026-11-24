//! Member profiles as a group shows them: what its direct-messages preference lets through (Internal.hs `redactedMemberProfile`, `memberInfo`; Subscriber.hs `processMemberProfileUpdate`).

use serde_json::json;

use crate::types::Request;

use super::members::acpt;
use super::members_support::{link, sent_on};
use crate::chat::ChatEvent;
use crate::chat::group::GroupMemberRole;
use crate::chat::group_prefs::{FeatureEnabled, GroupFeature, GroupPreferences};
use crate::chat::profile::Profile;
use crate::engine::agent_port::{AgentEvent, ConnId};
use crate::engine::tests::T;

/// The user owns "team" with bob and carol; `direct` is the group's direct-messages preference.
fn team(t: &mut T, direct: Option<GroupFeature>) -> (i64, [(i64, ConnId); 2]) {
    let (g, m) = t.group("team", &["bob", "carol"], GroupMemberRole::Owner);
    let mut rec = t.e.group(g).unwrap();
    rec.profile.group_preferences = Some(GroupPreferences {
        direct_messages: direct,
        ..GroupPreferences::default()
    });
    t.e.save_group(rec).unwrap();
    (g, [m[0], m[1]])
}

fn on() -> Option<GroupFeature> {
    Some(GroupFeature::new(FeatureEnabled::On))
}

fn full_profile(name: &str) -> Profile {
    Profile {
        contact_link: Some(link(7)),
        contact_domain: Some(json!({"domain": "carol.example", "proof": "signed"})),
        ..Profile::new(name, "")
    }
}

fn carol_says_her_profile(t: &mut T, direct: Option<GroupFeature>) -> Profile {
    let (g, [_, (carol_id, carol)]) = team(t, direct);
    t.peer_says(
        &carol,
        1,
        ChatEvent::Info {
            profile: full_profile("carol"),
            member_key: None,
        },
    );
    t.e.member(g, carol_id).unwrap().profile
}

#[test]
fn a_members_contact_link_and_domain_stay_without_proof_where_direct_messages_are_on() {
    let mut t = T::started();
    let p = carol_says_her_profile(&mut t, on());
    assert_eq!(p.contact_link, Some(link(7)));
    assert_eq!(p.contact_domain, Some(json!({"domain": "carol.example"})));
    assert_eq!(p.preferences, None);
}

#[test]
fn a_members_contact_link_and_domain_go_where_direct_messages_are_off() {
    let mut t = T::started();
    let p = carol_says_her_profile(&mut t, None);
    assert_eq!((p.contact_link, p.contact_domain), (None, None));
}

#[test]
fn direct_messages_for_admins_only_redact_a_plain_members_profile() {
    let mut t = T::started();
    let admins = Some(GroupFeature {
        role: Some("admin".into()),
        ..GroupFeature::new(FeatureEnabled::On)
    });
    let p = carol_says_her_profile(&mut t, admins);
    assert_eq!((p.contact_link, p.contact_domain), (None, None));
}

#[test]
fn an_introduction_carries_the_members_contact_link_where_direct_messages_are_on() {
    let mut t = T::started();
    let (g, [(bob_id, _), _]) = team(&mut t, on());
    let mut bob = t.e.member(g, bob_id).unwrap();
    bob.profile = full_profile("bob");
    t.e.save_member(bob).unwrap();
    let (dave, _) = t.friend("dave");
    t.req(Request::AddMember {
        group: g,
        friend: dave,
    });
    let m =
        t.e.state
            .members_of(g)
            .find(|m| m.friend == Some(dave))
            .unwrap()
            .clone();
    let conn = m.conn.unwrap();
    t.agent(AgentEvent::Invitation {
        conn,
        full: link(1),
        short: None,
    });
    let info = acpt(&t, g, m.id, "dave");
    t.agent(AgentEvent::Conf {
        conn,
        conn_info: info,
    });
    t.agent(AgentEvent::Con { conn });
    let bob_intro = sent_on(&t, conn).into_iter().find_map(|e| match e {
        ChatEvent::GrpMemIntro { member_info, .. } if member_info.profile.display_name == "bob" => {
            Some(member_info.profile)
        }
        _ => None,
    });
    let p = bob_intro.unwrap();
    assert_eq!(p.contact_link, Some(link(7)));
    assert_eq!(p.contact_domain, Some(json!({"domain": "carol.example"})));
}
