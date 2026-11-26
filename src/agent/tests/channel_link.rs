//! A channel's link against the in-memory server: made under the caller's root key and id, read without joining, and its relays published with LSET.
use super::support::{WAIT, rig, wait_for};
use crate::agent::client::AgentEvent;
use crate::agent::connection::LinkLookup;
use crate::agent::links::{ConnShortLink, ContactConnType};
use crate::agent::short_link::OwnerAuth;
use crate::agent::testing::{Node, pump};
use crate::crypto::{Alg, KeyPair, sha256};

fn look(b: &mut Node, link: &str) -> LinkLookup {
    let l = ConnShortLink::parse(link).unwrap();
    let conn = b.agent.lookup_link(&mut b.store, &l).unwrap();
    pump(&mut [&mut *b], WAIT, |n| {
        n[0].take(|e| match e {
            AgentEvent::LinkData { conn: c, data } if *c == conn => Some((**data).clone()),
            _ => None,
        })
    })
}

#[test]
fn a_channel_link_names_its_root_id_and_owner_and_lset_publishes_its_relays() {
    let r = rig();
    let (mut a, mut b) = (r.node("a"), r.node("b"));
    let root = KeyPair::generate(Alg::Ed25519).unwrap();
    let id = sha256(root.public.as_bytes()).to_vec();
    let member = KeyPair::generate(Alg::Ed25519).unwrap();
    let owner = OwnerAuth {
        owner_id: b"owner-member-id!".to_vec(),
        auth_owner_sig: root
            .private
            .sign(&[b"owner-member-id!".as_slice(), &member.public.to_der()].concat())
            .unwrap(),
        owner_key: member.public.clone(),
    };
    let seen = std::rc::Rc::new(std::cell::RefCell::new(String::new()));
    let s2 = seen.clone();
    let conn = a
        .agent
        .create_channel_link(
            &mut a.store,
            &root.private,
            id.clone(),
            vec![owner.clone()],
            move |link: &str| {
                *s2.borrow_mut() = link.to_owned();
                link.as_bytes().to_vec()
            },
            r#"{"groupLinkId":"AAAAAAAAAAAAAAAAAAAAAA=="}"#.into(),
        )
        .unwrap();
    let short = pump(&mut [&mut a], WAIT, |n| {
        n[0].take(|e| match e {
            AgentEvent::Address { short, .. } => short.clone(),
            _ => None,
        })
    });
    // The user data could name the link before it existed: the same one the server answers for.
    assert_eq!(*seen.borrow(), short);
    let parsed = ConnShortLink::parse(&short).unwrap();
    assert!(matches!(
        parsed,
        ConnShortLink::Contact {
            contact_type: ContactConnType::Channel,
            ..
        }
    ));
    let got = look(&mut b, &short);
    assert_eq!(got.entity_id.as_deref(), Some(id.as_slice()));
    assert_eq!(got.root_key, root.public);
    assert_eq!(got.owners, vec![owner.clone()]);
    assert!(got.relays.is_empty());
    assert_eq!(got.user_data, short.as_bytes());

    let relay =
        ConnShortLink::parse("https://localhost/g#AQIDBAUGBwgBAgMEBQYHCAECAwQFBgcIAQIDBAUGBwg")
            .unwrap();
    a.agent
        .set_link_data(
            &mut a.store,
            conn,
            vec![owner.clone()],
            vec![relay.clone()],
            b"v2".to_vec(),
        )
        .unwrap();
    wait_for(&mut [&mut a], 0, |e| {
        matches!(e, AgentEvent::LinkUpdated { conn: c } if *c == conn).then_some(())
    });
    let got = look(&mut b, &short);
    assert_eq!(got.relays.len(), 1);
    assert_eq!(got.user_data, b"v2");
}

#[test]
fn a_second_update_made_while_the_first_is_out_is_the_one_kept() {
    let r = rig();
    let (mut a, mut b) = (r.node("a"), r.node("b"));
    let root = KeyPair::generate(Alg::Ed25519).unwrap();
    let id = sha256(root.public.as_bytes()).to_vec();
    let conn = a
        .agent
        .create_channel_link(
            &mut a.store,
            &root.private,
            id,
            Vec::new(),
            |_: &str| b"v1".to_vec(),
            "{}".into(),
        )
        .unwrap();
    let short = pump(&mut [&mut a], WAIT, |n| {
        n[0].take(|e| match e {
            AgentEvent::Address { short, .. } => short.clone(),
            _ => None,
        })
    });
    // Two updates before any answer: only the second's answer reports it stored.
    a.agent
        .set_link_data(&mut a.store, conn, Vec::new(), Vec::new(), b"v2".to_vec())
        .unwrap();
    a.agent
        .set_link_data(&mut a.store, conn, Vec::new(), Vec::new(), b"v3".to_vec())
        .unwrap();
    wait_for(&mut [&mut a], 0, |e| {
        matches!(e, AgentEvent::LinkUpdated { .. }).then_some(())
    });
    assert_eq!(look(&mut b, &short).user_data, b"v3");
}
