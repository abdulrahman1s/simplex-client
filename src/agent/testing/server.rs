//! An in-memory SMP server for the agent tests: queues, subscriptions and checked command authorisation, over plain blocks.
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use crossbeam_channel::Sender;

use super::queues::Queues;
use crate::crypto::{Alg, KeyPair};
use crate::smp::SmpServer;
use crate::smp::tests::certs::{Identity, identity};

pub struct Session {
    pub tx: Sender<Vec<u8>>,
    pub closed: Arc<AtomicBool>,
    pub session_id: Vec<u8>,
}

pub struct Inner {
    pub queues: Queues,
    pub sessions: HashMap<u64, Session>,
    pub next_session: u64,
    pub refuse: bool,
    pub server_key: KeyPair,
    /// Command names in arrival order, for assertions.
    pub log: Vec<&'static str>,
    /// Connections clients opened to this server.
    pub connects: usize,
    /// As a proxy: the destination of each session PRXY opened, by session id. Eviction: none, a test's lifetime.
    pub relayed: HashMap<Vec<u8>, Arc<FakeSmp>>,
    /// As a destination: the sessions proxies opened here. Eviction: none, a test's lifetime.
    pub proxy_sessions: HashSet<Vec<u8>>,
    /// As a destination: the commands that came forwarded by a proxy, in order.
    pub forwarded: Vec<String>,
    /// As a destination: refuse proxies as a relay without forwarding support would.
    pub no_forwarding: bool,
}

pub struct FakeSmp {
    pub version: u16,
    pub inner: Mutex<Inner>,
    pub identity: Identity,
    pub host: String,
    /// The servers this one reaches as a proxy.
    pub net: Mutex<Vec<Arc<FakeSmp>>>,
}

impl FakeSmp {
    pub fn new() -> Arc<Self> {
        Self::at("fake.test")
    }

    /// A server whose address is `host` and the key hash of its own identity, as PKEY is checked against.
    pub fn at(host: &str) -> Arc<Self> {
        Arc::new(Self {
            identity: identity(),
            host: host.to_owned(),
            net: Mutex::new(Vec::new()),
            version: 23,
            inner: Mutex::new(Inner {
                queues: Queues {
                    quota: 128,
                    ..Queues::default()
                },
                sessions: HashMap::new(),
                next_session: 0,
                refuse: false,
                server_key: KeyPair::generate(Alg::X25519).unwrap(),
                log: Vec::new(),
                connects: 0,
                relayed: HashMap::new(),
                proxy_sessions: HashSet::new(),
                forwarded: Vec::new(),
                no_forwarding: false,
            }),
        })
    }

    pub fn address(&self) -> SmpServer {
        let hash = crate::encoding::b64_encode(&self.identity.key_hash, true);
        format!("smp://{hash}@{}", self.host).parse().unwrap()
    }

    pub fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap()
    }

    /// Cuts every connection, as a network failure would; with `refuse`, new ones fail until `refuse(false)`.
    pub fn drop_links(&self) {
        let mut i = self.lock();
        let ids: Vec<u64> = i.sessions.keys().copied().collect();
        for id in ids {
            i.end_session(id);
        }
    }

    pub fn refuse(&self, on: bool) {
        self.lock().refuse = on;
    }

    pub fn count(&self, name: &str) -> usize {
        self.lock().log.iter().filter(|n| **n == name).count()
    }
}

impl Inner {
    pub fn end_session(&mut self, id: u64) {
        if let Some(s) = self.sessions.remove(&id) {
            s.closed.store(true, Ordering::Release);
        }
        for q in self.queues.by_rcv.values_mut() {
            if q.subscriber == Some(id) {
                q.subscriber = None;
                q.delivered = None;
            }
        }
    }
}
