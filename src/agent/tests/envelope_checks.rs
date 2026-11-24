//! The reference's checks on what a relay can rewrite: the envelope's client version, and the server an invitation's short link names.
use super::support::{WAIT, connect, rig, wait_for};
use crate::agent::client::{AgentEvent, ConnId};
use crate::agent::delivery::seal;
use crate::agent::error::AgentError;
use crate::agent::links::{ConnShortLink, ConnectionLink};
use crate::agent::testing::{Node, pump};
use crate::agent::{AgentMsgEnvelope, SmpServer};
use crate::encoding::{Encode, Version};

fn error_on(n: &mut [&mut Node], who: usize, c: ConnId) -> AgentError {
    wait_for(n, who, |e| match e {
        AgentEvent::Err { conn, err } if *conn == c => Some(err.clone()),
        _ => None,
    })
}

#[test]
fn an_envelope_above_the_supported_client_versions_is_refused() {
    let r = rig();
    let (mut a, mut b) = (r.node("a"), r.node("b"));
    let (ca, cb) = connect(&mut a, &mut b, false, false);
    let mut sq = b.agent.conn(cb).unwrap().rec.snd.clone().unwrap();
    sq.client_version = Version(5);
    let rcv_id = a.agent.conn(ca).unwrap().rec.rcv.clone().unwrap().rcv_id;
    let msg = AgentMsgEnvelope::Message {
        agent_version: b.agent.conn(cb).unwrap().rec.agent_version,
        enc_agent_message: b"anything".to_vec(),
    };
    r.smp
        .inject(&rcv_id, seal(&sq, &msg.to_bytes().unwrap(), false).unwrap());
    assert_eq!(error_on(&mut [&mut a, &mut b], 0, ca), AgentError::Version);
}

#[test]
fn an_invitation_short_link_must_name_the_server_its_data_names() {
    let r = rig();
    let (mut a, mut b) = (r.node("a"), r.node("b"));
    a.agent
        .create_invitation(&mut a.store, false, true)
        .unwrap();
    let short = pump(&mut [&mut a, &mut b], WAIT, |n| {
        n[0].take(|e| match e {
            AgentEvent::Invitation { short, .. } => short.clone(),
            _ => None,
        })
    });
    let ConnectionLink::Short(link) = ConnectionLink::parse(&short).unwrap() else {
        panic!("a short link")
    };
    let cb = b
        .agent
        .join(
            &mut b.store,
            &ConnectionLink::Short(link.clone()),
            b"bob".to_vec(),
            false,
        )
        .unwrap();
    // LKEY went to the real server; the link the answer is checked against now names another.
    let key = crate::encoding::b64_encode(&[1; 32], true);
    let other: SmpServer = format!("smp://{key}@other.test").parse().unwrap();
    let moved: ConnShortLink = link.with_server(other);
    b.agent.conns.get_mut(&cb).unwrap().rec.link = Some(moved.to_uri());
    assert_eq!(
        error_on(&mut [&mut b, &mut a], 0, cb),
        AgentError::Link("different address".into())
    );
}
