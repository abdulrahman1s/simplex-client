//! What the channel tests share: a channel's and a relay's link data, a subscriber joined through two relays, an owner with one relay, and posts as the channel.

use crate::agent::connection::LinkLookup;
use crate::agent::links::{ConnShortLink, ConnectionLink, ConnectionRequestUri};
use crate::agent::short_link::{OwnerAuth, encode_short_link_data};
use crate::chat::container::MsgContainer;
use crate::chat::element::encode_forward_element;
use crate::chat::forward::{FwdSender, GrpMsgForward};
use crate::chat::group::{GroupLinkInvitation, GroupMemberRole, MemberIdRole};
use crate::chat::ids::{MemberId, b64_encode};
use crate::chat::{ChatEvent, ChatMessage};
use crate::crypto::{Alg, KeyPair, sha256};
use crate::engine::agent_port::{AgentEvent, ConnId};
use crate::types::Request;
use crate::types::model::ChatRelay;

use super::super::{Call, T};

pub(super) const ADDRESS: &str = "simplex:/contact#/?v=6-8&smp=smp%3A%2F%2FAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQEBAQE%3D%40fake.test%2F-FWjHkx7I4TOSuF3q4l2S4jD2BgprZRG%23%2F%3Fv%3D1-4%26dh%3DMCowBQYDK2VuAyEAQWIOFJ7ZTMqTFfTsHuyRYNDwszkpHQfRxObZXAWAIRU%253D%26q%3Dc";
pub(super) const CHANNEL: &str = "https://fake.test/c#AQIDBAUGBwgBAgMEBQYHCAECAwQFBgcIAQIDBAUGBwg";
pub(super) const RELAYS: [&str; 2] = [
    "https://fake.test/g#AgIDBAUGBwgBAgMEBQYHCAECAwQFBgcIAQIDBAUGBwg",
    "https://fake.test/g#AwIDBAUGBwgBAgMEBQYHCAECAwQFBgcIAQIDBAUGBwg",
];
pub(super) const TAG: &str = "zz.app";

pub(super) fn request() -> ConnectionRequestUri {
    let ConnectionLink::Full(r) = ConnectionLink::parse(ADDRESS).unwrap() else {
        panic!()
    };
    r
}

/// A channel as its link shows it: id, profile and the relays named.
pub(super) struct Channel {
    pub root: KeyPair,
    pub id: Vec<u8>,
}

impl Channel {
    pub(super) fn new() -> Self {
        let root = KeyPair::generate(Alg::Ed25519).unwrap();
        let id = sha256(root.public.as_bytes()).to_vec();
        Self { root, id }
    }

    pub(super) fn profile(&self) -> serde_json::Value {
        serde_json::json!({
            "displayName": "news", "fullName": "",
            "publicGroup": {"groupType": "channel", "groupLink": CHANNEL, "publicGroupId": b64_encode(&self.id)}
        })
    }

    pub(super) fn link_data(&self, entity: &[u8]) -> LinkLookup {
        let data = serde_json::json!({"groupProfile": self.profile(), "publicGroupData": {"publicMemberCount": 1}});
        LinkLookup {
            root_key: self.root.public.clone(),
            entity_id: Some(entity.to_vec()),
            owners: Vec::<OwnerAuth>::new(),
            relays: RELAYS
                .iter()
                .map(|r| ConnShortLink::parse(r).unwrap())
                .collect(),
            user_data: encode_short_link_data(&data).unwrap().0,
            conn_req: request(),
        }
    }
}

pub(super) fn relay_data(relay: &KeyPair, member_id: &[u8]) -> LinkLookup {
    let data = serde_json::json!({"relayProfile": {"displayName": "relay", "fullName": ""}});
    LinkLookup {
        root_key: relay.public.clone(),
        entity_id: Some(member_id.to_vec()),
        owners: Vec::new(),
        relays: Vec::new(),
        user_data: encode_short_link_data(&data).unwrap().0,
        conn_req: request(),
    }
}

pub(super) fn lookups(t: &T) -> Vec<String> {
    (t.e.agent.calls.iter())
        .filter_map(|c| match c {
            Call::LookupLink { link } => Some(link.clone()),
            _ => None,
        })
        .collect()
}

/// The last lookup connection the engine keeps (fake agent numbers connections in order).
pub(super) fn last_lookup(t: &T) -> ConnId {
    *t.e.state
        .conns
        .iter()
        .filter(|(_, c)| matches!(c.kind, crate::engine::records::ConnKind::Lookup(_)))
        .map(|(k, _)| k)
        .max()
        .unwrap()
}

/// A subscriber joined through both relays and connected: the group and each relay's conn.
pub(super) fn subscribed(t: &mut T, ch: &Channel) -> (i64, Vec<ConnId>) {
    t.e.data_tags.insert(TAG.into());
    t.req(Request::Connect(CHANNEL.into()));
    let conn = last_lookup(t);
    t.agent(AgentEvent::LinkData {
        conn,
        data: Box::new(ch.link_data(&ch.id)),
    });
    let group = *t.e.state.groups.keys().next().unwrap();
    let mut conns = Vec::new();
    for (i, _) in RELAYS.iter().enumerate() {
        let relay = KeyPair::generate(Alg::Ed25519).unwrap();
        let member_id = format!("relay-member-{i}").into_bytes();
        let conn =
            *t.e.state
                .conns
                .iter()
                .filter(|(_, c)| matches!(c.kind, crate::engine::records::ConnKind::Lookup(_)))
                .map(|(k, _)| k)
                .min()
                .unwrap();
        t.agent(AgentEvent::LinkData {
            conn,
            data: Box::new(relay_data(&relay, &member_id)),
        });
        let joined = *t.e.state.conns.keys().max().unwrap();
        let g = t.e.state.groups[&group].clone();
        let inv = GroupLinkInvitation {
            from_member: MemberIdRole {
                member_id: MemberId(member_id.clone()),
                member_role: GroupMemberRole::Relay,
            },
            from_member_name: "relay".into(),
            from_member_key: Some(b64_encode(&relay.public.to_der())),
            invited_member: MemberIdRole {
                member_id: g.my_member_id.clone(),
                member_role: GroupMemberRole::Observer,
            },
            group_profile: serde_json::from_value(ch.profile()).unwrap(),
            accepted: None,
            business: None,
            group_size: None,
        };
        let info = ChatMessage::without_id(ChatEvent::GrpLinkInv(inv))
            .to_json()
            .unwrap();
        t.agent(AgentEvent::Conf {
            conn: joined,
            conn_info: info,
        });
        t.agent(AgentEvent::Con { conn: joined });
        conns.push(joined);
    }
    (group, conns)
}

pub(super) fn post(id: &[u8], c: MsgContainer) -> Vec<u8> {
    let mut msg = ChatMessage::new(ChatEvent::MsgNew(c)).unwrap();
    msg.msg_id = Some(crate::chat::ids::SharedMsgId(id.to_vec()));
    let fwd = GrpMsgForward {
        sender: FwdSender::Channel,
        broker_ts: "2026-11-26T12:00:00Z".into(),
    };
    encode_forward_element(&fwd, &msg.to_json().unwrap()).unwrap()
}

pub(super) fn data_post(id: &[u8]) -> Vec<u8> {
    let content = crate::chat::app_data::content(
        TAG,
        &serde_json::Map::from_iter([("k".to_owned(), serde_json::json!(1))]),
        "post",
    );
    let mut c = MsgContainer::simple(content);
    c.as_group = Some(true);
    post(id, c)
}

/// An owner with one relay that took the roster.
pub(super) fn owner_with_relay(t: &mut T) -> (i64, i64, ConnId) {
    let relay = ChatRelay {
        address: RELAYS[0].replace("/g#", "/r#"),
        name: "relay".into(),
    };
    t.req(Request::SetChatRelays(vec![relay]));
    t.req(Request::NewChannel("news".into()));
    let (group, g) =
        t.e.state
            .groups
            .iter()
            .next()
            .map(|(k, v)| (*k, v.clone()))
            .unwrap();
    let link_conn = g.link_conn.unwrap();
    t.agent(AgentEvent::Address {
        conn: link_conn,
        full: ADDRESS.into(),
        short: Some(CHANNEL.into()),
    });
    let conn = last_lookup(t);
    let member = t.e.state.groups[&group].channel.as_ref().unwrap().relays[0].member;
    let member_id = t.e.state.members[&(group, member)].member_id.0.clone();
    t.agent(AgentEvent::LinkData {
        conn,
        data: Box::new(relay_data(
            &KeyPair::generate(Alg::Ed25519).unwrap(),
            &member_id,
        )),
    });
    let joined = t.e.state.members[&(group, member)].conn.unwrap();
    let acpt = ChatMessage::without_id(ChatEvent::GrpRelayAcpt {
        relay_link: RELAYS[1].into(),
        relay_cap: Default::default(),
    });
    t.agent(AgentEvent::Conf {
        conn: joined,
        conn_info: acpt.to_json().unwrap(),
    });
    let conn = last_lookup(t);
    let relay = KeyPair::generate(Alg::Ed25519).unwrap();
    t.agent(AgentEvent::LinkData {
        conn,
        data: Box::new(relay_data(&relay, &member_id)),
    });
    t.agent(AgentEvent::Con { conn: joined });
    let ack = ChatMessage::new(ChatEvent::GrpRosterAck {
        version: 0,
        error: None,
    })
    .unwrap();
    t.agent(AgentEvent::Msg {
        conn: joined,
        msg_id: 1,
        body: ack.to_json().unwrap(),
        integrity: crate::agent::MsgIntegrity::Ok,
        broker_ts: 0,
    });
    (group, member, link_conn)
}
