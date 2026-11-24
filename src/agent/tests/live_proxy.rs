//! Two native agents on two real smp-servers, each sending only through its own relay. Start two local smp-servers and run
//! `SIMPLEX_TEST_SMP=smp://…@127.0.0.1:<port1> SIMPLEX_TEST_SMP2=smp://…@127.0.0.1:<port2> cargo test --lib agent::tests::live_proxy -- --ignored`.
use std::sync::{Arc, Mutex};

use super::live::live_server;
use super::support::{connect, next_msg, pq_of, wait_for};
use crate::agent::client::{AgentConfig, AgentEvent, RcptStatus, SystemWallClock};
use crate::agent::testing::{Dir, Node};
use crate::smp::{Closer, ConnConfig, Connector, SmpError, SmpServer, TlsConnector, Wire};
use crate::types::ProxyMode;

fn second_server() -> SmpServer {
    let Ok(addr) = std::env::var("SIMPLEX_TEST_SMP2") else {
        panic!("SIMPLEX_TEST_SMP2 is not set")
    };
    addr.parse().unwrap()
}

/// TLS, remembering every server the node opened a connection to.
struct Recording(Mutex<Vec<SmpServer>>);

impl Connector for Recording {
    fn connect(
        &self,
        server: &SmpServer,
        host: &str,
        cfg: &ConnConfig,
        slot: &dyn Fn(Arc<dyn Closer>),
    ) -> Result<Wire, SmpError> {
        self.0.lock().unwrap().push(server.clone());
        TlsConnector.connect(server, host, cfg, slot)
    }
}

fn node(dir: &Dir, name: &str, own: SmpServer) -> (Node, Arc<Recording>) {
    let mut cfg = AgentConfig::new(vec![own]);
    cfg.proxy_mode = ProxyMode::Always;
    cfg.smp.reconnect_min = std::time::Duration::from_millis(200);
    let rec = Arc::new(Recording(Mutex::new(Vec::new())));
    let n = Node::open(
        dir,
        name,
        cfg,
        Arc::clone(&rec) as _,
        Arc::new(SystemWallClock),
    );
    (n, rec)
}

#[test]
#[ignore = "needs two local smp-servers; set SIMPLEX_TEST_SMP and SIMPLEX_TEST_SMP2"]
fn two_agents_on_two_relays_talk_only_through_their_own_relays() {
    let (one, two) = (live_server(), second_server());
    let dir = Dir::new();
    let (mut a, a_saw) = node(&dir, "a", one.clone());
    let (mut b, b_saw) = node(&dir, "b", two.clone());
    let (ca, cb) = connect(&mut a, &mut b, false, true);
    let sent = a
        .agent
        .send(&mut a.store, ca, pq_of(&a.agent, ca), b"via one".to_vec());
    let sent = sent.unwrap();
    let (id, body) = next_msg(&mut [&mut a, &mut b], 1, cb);
    assert_eq!(body, b"via one");
    b.agent.ack(&mut b.store, cb, id, Some(Vec::new())).unwrap();
    let rcvd = wait_for(&mut [&mut a, &mut b], 0, |e| match e {
        AgentEvent::Rcvd {
            msg_id, receipt, ..
        } if *msg_id == sent => Some(receipt.status),
        _ => None,
    });
    assert_eq!(rcvd, RcptStatus::Ok);
    b.agent
        .send(&mut b.store, cb, pq_of(&b.agent, cb), b"via two".to_vec())
        .unwrap();
    assert_eq!(next_msg(&mut [&mut a, &mut b], 0, ca).1, b"via two");
    assert!(
        a_saw.0.lock().unwrap().iter().all(|s| *s == one),
        "a reached only its relay"
    );
    assert!(
        b_saw.0.lock().unwrap().iter().all(|s| *s == two),
        "b reached only its relay"
    );
}
