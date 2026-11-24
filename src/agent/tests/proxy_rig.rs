//! Two agents on two in-memory relays that also act as proxies and destinations for each other.
use std::sync::Arc;
use std::sync::atomic::AtomicI64;
use std::time::Duration;

use super::support::pq_of;
use crate::agent::client::{AgentConfig, AgentMsgId, ConnId};
use crate::agent::testing::{Dir, FakeSmp, NetConnector, Node, TestClock};
use crate::smp::{ConnConfig, SmpServer};
use crate::types::ProxyMode;

pub struct Net {
    pub a: Arc<FakeSmp>,
    pub b: Arc<FakeSmp>,
    dir: Dir,
    pub clock: Arc<TestClock>,
}

pub fn net() -> Net {
    let (a, b) = (FakeSmp::at("a.test"), FakeSmp::at("b.test"));
    for s in [&a, &b] {
        *s.net.lock().unwrap() = vec![Arc::clone(&a), Arc::clone(&b)];
    }
    Net {
        a,
        b,
        dir: Dir::new(),
        clock: Arc::new(TestClock(AtomicI64::new(1_000_000))),
    }
}

impl Net {
    pub fn node(&self, name: &str, servers: &[&Arc<FakeSmp>], mode: ProxyMode) -> Node {
        let servers: Vec<SmpServer> = servers.iter().map(|s| s.address()).collect();
        let mut cfg = AgentConfig::new(servers);
        cfg.proxy_mode = mode;
        cfg.smp = ConnConfig {
            ping_interval: Duration::ZERO,
            reconnect_min: Duration::from_millis(20),
            reconnect_max: Duration::from_millis(50),
            ..ConnConfig::default()
        };
        let connector = Arc::new(NetConnector(vec![Arc::clone(&self.a), Arc::clone(&self.b)]));
        Node::open(
            &self.dir,
            name,
            cfg,
            connector,
            Arc::clone(&self.clock) as _,
        )
    }
}

pub fn forwarded(s: &FakeSmp, name: &str) -> usize {
    s.lock().forwarded.iter().filter(|n| *n == name).count()
}

pub fn send(n: &mut Node, conn: ConnId, body: &[u8]) -> AgentMsgId {
    let pq = pq_of(&n.agent, conn);
    n.agent.send(&mut n.store, conn, pq, body.to_vec()).unwrap()
}
