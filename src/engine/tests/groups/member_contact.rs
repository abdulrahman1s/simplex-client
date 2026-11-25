//! ContactMember: a one-time link to a group member, sent over the group connection (Commands.hs `APICreateMemberContact`, `APISendMemberContactInvitation`).

use crate::types::model::{ChatRef, MessageBody};
use crate::types::{Request, RequestKind};

use super::messages::{failed, sent_to};
use crate::chat::ChatEvent;
use crate::chat::group::GroupMemberRole;
use crate::chat::group_prefs::{FeatureEnabled, GroupFeature};
use crate::engine::agent_port::AgentEvent;
use crate::engine::tests::T;

fn direct_on(t: &mut T, g: i64) {
    let mut group = t.e.group(g).unwrap();
    let prefs = group
        .profile
        .group_preferences
        .get_or_insert_with(Default::default);
    prefs.direct_messages = Some(GroupFeature::new(FeatureEnabled::On));
    t.e.save_group(group).unwrap();
}

#[test]
fn a_member_gets_the_link_over_the_group_connection_once_it_is_made() {
    let mut t = T::started();
    let (g, m) = t.group("team", &["bob"], GroupMemberRole::Owner);
    let (bob, bob_conn) = m[0];
    direct_on(&mut t, g);
    t.req(Request::ContactMember {
        group: g,
        member: bob,
        text: Some("hi bob".into()),
    });
    let friend = t.e.member(g, bob).unwrap().friend.unwrap();
    let f = t.e.state.friends[&friend].clone();
    assert!(!f.ready && f.name == "bob");
    assert!(sent_to(&t, &bob_conn).is_empty(), "not before the link");
    t.agent(AgentEvent::Invitation {
        conn: f.conn,
        full: "simplex:/invitation#/?v=2-7&smp=x".into(),
        short: None,
    });
    let told = sent_to(&t, &bob_conn);
    let [
        ChatEvent::GrpDirectInv {
            conn_req, content, ..
        },
    ] = told.as_slice()
    else {
        panic!("{told:?}")
    };
    assert!(conn_req.starts_with("simplex:/invitation"));
    assert_eq!(content.as_ref().map(|c| c.text()), Some("hi bob"));
    let items = t.e.items_page(ChatRef::Direct(friend), None, 5).unwrap();
    assert!(
        items
            .iter()
            .any(|i| i.body == MessageBody::Text("hi bob".into()))
    );
}

#[test]
fn contacting_a_member_needs_direct_messages_on_and_no_contact_yet() {
    let mut t = T::started();
    let (g, m) = t.group("team", &["bob"], GroupMemberRole::Owner);
    let ask = |t: &mut T| {
        t.req(Request::ContactMember {
            group: g,
            member: m[0].0,
            text: None,
        })
    };
    assert!(failed(&ask(&mut t), RequestKind::Connect), "off by default");
    direct_on(&mut t, g);
    assert!(!failed(&ask(&mut t), RequestKind::Connect));
    assert!(failed(&ask(&mut t), RequestKind::Connect), "already asked");
}
