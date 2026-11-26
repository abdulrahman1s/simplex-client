//! An owner's relays on the fake agent: a relay test's three outcomes, a relay added to a live channel and published with the first, the announce it gets once someone subscribed, and a subscriber joining a relay an owner announced.

use crate::agent::connection::LinkLookup;
use crate::agent::links::ConnShortLink;
use crate::agent::short_link::encode_short_link_data;
use crate::chat::element::{encode_forward_element, encode_signed_element};
use crate::chat::forward::{FwdSender, GrpMsgForward};
use crate::chat::group::{GroupMemberRole, MemberInfo};
use crate::chat::ids::b64_encode;
use crate::chat::profile::Profile;
use crate::chat::signed::{ChatBinding, MsgSignature, SignedMsg};
use crate::chat::{ChatEvent, ChatMessage, parse_chat_elements};
use crate::crypto::{Alg, KeyPair};
use crate::engine::agent_port::{AgentEvent, ConnId};
use crate::engine::records::RelayStatus;
use crate::types::model::{ChatRelay, RelayTestStep};
use crate::types::{Event, Request};

use super::super::{Call, T};
use super::fixtures::*;

const ADDR: &str = "https://fake.test/r#BAIDBAUGBwgBAgMEBQYHCAECAwQFBgcIAQIDBAUGBwg";
const SECOND: &str = "https://fake.test/r#BQIDBAUGBwgBAgMEBQYHCAECAwQFBgcIAQIDBAUGBwg";
const SECOND_LINK: &str = "https://fake.test/g#BgIDBAUGBwgBAgMEBQYHCAECAwQFBgcIAQIDBAUGBwg";

/// A relay test past its lookup: the relay's key, the test connection and the challenge sent.
fn testing(t: &mut T) -> (KeyPair, ConnId, Vec<u8>) {
    t.req(Request::TestChatRelay(ADDR.into()));
    let relay = KeyPair::generate(Alg::Ed25519).unwrap();
    let data = serde_json::json!({"relayProfile": {"displayName": "relay one", "fullName": ""}});
    let lookup = LinkLookup {
        root_key: relay.public.clone(),
        entity_id: None,
        owners: Vec::new(),
        relays: Vec::new(),
        user_data: encode_short_link_data(&data).unwrap().0,
        conn_req: request(),
    };
    t.agent(AgentEvent::LinkData {
        conn: last_lookup(t),
        data: Box::new(lookup),
    });
    let Some(Call::Join { conn_info, .. }) = t.e.agent.calls.last().cloned() else {
        panic!("the address is joined")
    };
    let p = parse_chat_elements(&conn_info).remove(0).unwrap();
    assert!(p.signed.is_none(), "the challenge goes unsigned");
    let ChatEvent::GrpRelayTest {
        challenge,
        signature: None,
    } = p.message.event
    else {
        panic!("{:?}", p.message.event)
    };
    assert_eq!(challenge.len(), 32);
    let conn = *t.e.state.conns.keys().max().unwrap();
    (relay, conn, challenge)
}

fn answer(t: &mut T, conn: ConnId, challenge: Vec<u8>, signature: Option<Vec<u8>>) -> Vec<Event> {
    let info = ChatMessage::without_id(ChatEvent::GrpRelayTest {
        challenge,
        signature,
    });
    t.agent(AgentEvent::Conf {
        conn,
        conn_info: info.to_json().unwrap(),
    })
}

fn result(out: &[Event]) -> crate::types::model::RelayTestResult {
    out.iter()
        .find_map(|e| match e {
            Event::RelayTest(r) => Some(r.clone()),
            _ => None,
        })
        .expect("a relay test result")
}

#[test]
fn a_relay_that_signs_the_challenge_with_its_address_key_passes_and_the_test_connection_goes() {
    let mut t = T::started();
    let (relay, conn, challenge) = testing(&mut t);
    let sig = relay.private.sign(&challenge).unwrap().as_bytes().to_vec();
    let r = result(&answer(&mut t, conn, challenge, Some(sig)));
    assert_eq!((r.name.as_deref(), r.failure), (Some("relay one"), None));
    assert!(t.e.agent.calls.contains(&Call::Delete { conn }));
    assert!(!t.e.state.conns.contains_key(&conn));
}

#[test]
fn a_relay_test_fails_on_a_wrong_or_missing_signature_and_on_no_answer() {
    let mut t = T::started();
    let (_, conn, challenge) = testing(&mut t);
    let other = KeyPair::generate(Alg::Ed25519).unwrap();
    let sig = other.private.sign(&challenge).unwrap().as_bytes().to_vec();
    let f = result(&answer(&mut t, conn, challenge, Some(sig)))
        .failure
        .unwrap();
    assert_eq!(
        (f.step, f.error.as_str()),
        (RelayTestStep::Verify, "invalid signature")
    );

    let (_, conn, challenge) = testing(&mut t);
    let f = result(&answer(&mut t, conn, challenge, None))
        .failure
        .unwrap();
    assert_eq!(
        (f.step, f.error.as_str()),
        (RelayTestStep::Verify, "no signature in response")
    );

    let (_, conn, _) = testing(&mut t);
    t.clock.advance(std::time::Duration::from_secs(41));
    t.e.tick();
    let f = result(&t.out()).failure.unwrap();
    assert_eq!(
        (f.step, f.error.as_str()),
        (RelayTestStep::WaitResponse, "timeout")
    );
    assert!(!t.e.state.conns.contains_key(&conn));
}

#[test]
fn a_relay_address_without_relay_data_fails_at_decoding_the_link() {
    let mut t = T::started();
    t.req(Request::TestChatRelay(ADDR.into()));
    let lookup = relay_data(&KeyPair::generate(Alg::Ed25519).unwrap(), b"x");
    let mut lookup = lookup;
    lookup.user_data = b"{}".to_vec();
    let out = t.agent(AgentEvent::LinkData {
        conn: last_lookup(&t),
        data: Box::new(lookup),
    });
    let f = result(&out).failure.unwrap();
    assert_eq!(
        (f.step, f.error.as_str()),
        (RelayTestStep::DecodeLink, "no relay address link data")
    );
}

fn published(t: &T) -> Vec<String> {
    (t.e.agent.calls.iter())
        .rev()
        .find_map(|c| match c {
            Call::SetLinkData { relays, .. } => Some(relays.clone()),
            _ => None,
        })
        .unwrap()
}

#[test]
fn a_relay_added_later_is_published_beside_the_first_and_announced_once_someone_subscribed() {
    let mut t = T::started();
    let (group, first, link_conn) = owner_with_relay(&mut t);
    t.agent(AgentEvent::LinkUpdated { conn: link_conn });
    // The first relay announces a subscriber to the owner (`x.grp.mem.new`).
    let first_conn = t.e.state.members[&(group, first)].conn.unwrap();
    let new = ChatMessage::new(ChatEvent::GrpMemNew {
        member_info: MemberInfo {
            member_id: crate::chat::ids::MemberId(b"subscriber-id-01".to_vec()),
            member_role: GroupMemberRole::Observer,
            v: None,
            profile: Profile::new("sub", ""),
            member_key: None,
        },
        scope: None,
    })
    .unwrap();
    t.agent(AgentEvent::Msg {
        conn: first_conn,
        msg_id: 2,
        body: new.to_json().unwrap(),
        integrity: crate::agent::MsgIntegrity::Ok,
        broker_ts: 0,
    });
    let second = ChatRelay {
        address: SECOND.into(),
        name: "second".into(),
    };
    let out = t.req(Request::AddChannelRelays {
        group,
        relays: vec![second.clone()],
    });
    assert!(
        out.iter().any(|e| matches!(e, Event::ChannelRelay { .. })),
        "{out:?}"
    );
    let (member, _) = bring_relay(&mut t, group, SECOND, SECOND_LINK);
    let status = |t: &T| {
        t.e.state.groups[&group]
            .channel
            .as_ref()
            .unwrap()
            .relay(member)
            .unwrap()
            .status
    };
    assert_eq!(status(&t), RelayStatus::AcknowledgedRoster);
    let both: Vec<_> = [RELAYS[1], SECOND_LINK]
        .iter()
        .map(|l| ConnShortLink::parse(l).unwrap().to_uri())
        .collect();
    assert_eq!(published(&t), both);
    let before = t.e.agent.sent().len();
    t.agent(AgentEvent::LinkUpdated { conn: link_conn });
    assert_eq!(status(&t), RelayStatus::Active);
    let sent = t.e.agent.sent();
    assert_eq!(
        sent.len(),
        before + 1,
        "the announce goes to the first relay only"
    );
    assert_eq!(sent[before].0, first_conn);
    let p = parse_chat_elements(&sent[before].1).remove(0).unwrap();
    assert!(p.signed.is_some(), "x.grp.relay.new is signed");
    assert_eq!(
        p.message.event,
        ChatEvent::GrpRelayNew {
            relay_link: SECOND_LINK.into()
        }
    );

    let again = t.req(Request::AddChannelRelays {
        group,
        relays: vec![second],
    });
    assert!(again.iter().any(|e| matches!(e, Event::Failed { detail, .. } if detail == "some relays are already in the group")), "{again:?}");
}

/// An owner's signed `x.grp.relay.new`, as a relay forwards it.
fn announced(ch: &Channel, link: &str, signed_by: &KeyPair) -> Vec<u8> {
    let msg = ChatMessage::new(ChatEvent::GrpRelayNew {
        relay_link: link.into(),
    })
    .unwrap();
    let body = msg.to_json().unwrap();
    let mut binding = vec![u8::try_from(ch.id.len()).unwrap()];
    binding.extend_from_slice(&ch.id);
    binding.push(u8::try_from(ch.owner_id.0.len()).unwrap());
    binding.extend_from_slice(&ch.owner_id.0);
    let mut sm = SignedMsg {
        binding: ChatBinding::Group,
        signatures: Vec::new(),
        signed_body: body,
    };
    let sig = signed_by.private.sign(&sm.signed_input(&binding)).unwrap();
    sm.signatures.push(MsgSignature {
        signature: sig.as_bytes().to_vec(),
    });
    let fwd = GrpMsgForward {
        sender: FwdSender::Member {
            member_id: ch.owner_id.clone(),
            member_name: "owner".into(),
        },
        broker_ts: "2026-11-26T12:00:00Z".into(),
    };
    encode_forward_element(&fwd, &encode_signed_element(&sm).unwrap()).unwrap()
}

#[test]
fn a_subscriber_joins_a_relay_its_owner_announced_and_not_one_a_forged_announce_names() {
    let mut t = T::started();
    let ch = Channel::new();
    let (group, conns) = subscribed(&mut t, &ch);
    let forged = announced(&ch, SECOND_LINK, &KeyPair::generate(Alg::Ed25519).unwrap());
    let out = t.agent(AgentEvent::Msg {
        conn: conns[0],
        msg_id: 30,
        body: forged,
        integrity: crate::agent::MsgIntegrity::Ok,
        broker_ts: 0,
    });
    assert!(
        lookups(&t)
            .iter()
            .all(|l| ConnShortLink::parse(l).unwrap().link_key()
                != ConnShortLink::parse(SECOND_LINK).unwrap().link_key())
    );
    assert!(
        out.iter().any(|e| matches!(e, Event::Messages(_))),
        "a bad signature notice: {out:?}"
    );
    let real = announced(&ch, SECOND_LINK, &ch.owner);
    t.agent(AgentEvent::Msg {
        conn: conns[0],
        msg_id: 31,
        body: real.clone(),
        integrity: crate::agent::MsgIntegrity::Ok,
        broker_ts: 0,
    });
    let joined = |t: &T| {
        lookups(t)
            .iter()
            .filter(|l| {
                ConnShortLink::parse(l).unwrap().link_key()
                    == ConnShortLink::parse(SECOND_LINK).unwrap().link_key()
            })
            .count()
    };
    assert_eq!(joined(&t), 1);
    // The other relay's copy of the same announce joins nothing more.
    t.agent(AgentEvent::Msg {
        conn: conns[1],
        msg_id: 32,
        body: real,
        integrity: crate::agent::MsgIntegrity::Ok,
        broker_ts: 0,
    });
    assert_eq!(joined(&t), 1);
    let relays = (t.e.state.members_of(group))
        .filter(|m| m.role == GroupMemberRole::Relay)
        .count();
    assert_eq!(relays, 3);
    let _ = b64_encode(&[]);
}
