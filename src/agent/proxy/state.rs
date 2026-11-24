//! The proxies our sends go through and their sessions with destination relays (Agent/Client.hs `smpProxiedRelays`, `SMPConnectedClient.proxiedRelays`).
use std::collections::HashMap;

use crate::agent::client::{ConnId, OpKind};
use crate::smp::proxy::Opener;
use crate::smp::{ProxiedRelay, Request, SmpServer};

/// A command for a destination, waiting for the proxy's session with it.
pub(crate) struct Waiting {
    pub conn: ConnId,
    pub kind: OpKind,
    /// As it would go to the destination directly.
    pub request: Request,
    /// The session was already asked for again after `NO_SESSION`.
    pub retried: bool,
}

pub(crate) enum Relay {
    /// PRXY is out; what comes for the destination meanwhile waits for its answer.
    Asking(Vec<Waiting>),
    Ready(ProxiedRelay),
    /// The proxy cannot forward to this relay at all (a host error); asked again after the proxy reconnects.
    Failed,
}

/// A PFWD out: what opens its answer, and what to send again or directly if it fails.
pub(crate) struct Forward {
    pub dest: SmpServer,
    pub session_id: Vec<u8>,
    pub opener: Opener,
    pub request: Request,
    pub retried: bool,
}

#[derive(Default)]
pub(crate) struct Proxies {
    /// The proxy each destination goes through, so that one destination sees one proxy. Eviction:
    /// the proxy going down, a failed PRXY other than a host error, and `retain`.
    pub chosen: HashMap<SmpServer, SmpServer>,
    /// Sessions by (proxy, destination). Eviction: the proxy going down, `NO_SESSION` for the same
    /// session, a failed PRXY other than a host error, and `retain`; `Asking` leaves only with its answer.
    pub relays: HashMap<(SmpServer, SmpServer), Relay>,
    /// PRXY tags. Eviction: the tagged reply, which smp sends for every request.
    pub asking: HashMap<u64, (SmpServer, SmpServer)>,
    /// PFWD tags. Eviction: the tagged reply.
    pub forwards: HashMap<u64, Forward>,
}

impl Proxies {
    /// `smpClientDisconnected`: the sessions were the proxy's, on a connection that is gone.
    pub(crate) fn on_down(&mut self, proxy: &SmpServer) {
        self.chosen.retain(|_, p| p != proxy);
        self.relays
            .retain(|(p, _), r| p != proxy || matches!(r, Relay::Asking(_)));
    }

    /// Forgets the destinations `keep` says no queue uses any more.
    pub(crate) fn retain(&mut self, keep: impl Fn(&SmpServer) -> bool) {
        self.chosen.retain(|d, _| keep(d));
        self.relays
            .retain(|(_, d), r| keep(d) || matches!(r, Relay::Asking(_)));
    }

    /// The proxies in use, whose connections must stay open.
    pub(crate) fn servers(&self) -> impl Iterator<Item = SmpServer> + '_ {
        let asking = self.asking.values().map(|(p, _)| p.clone());
        self.chosen.values().cloned().chain(asking)
    }

    /// Drops a session the proxy says it lost, unless a newer one has replaced it.
    pub(crate) fn drop_session(&mut self, proxy: &SmpServer, dest: &SmpServer, session_id: &[u8]) {
        let key = (proxy.clone(), dest.clone());
        if let Some(Relay::Ready(r)) = self.relays.get(&key)
            && r.session_id == session_id
        {
            self.relays.remove(&key);
        }
    }

    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }
}
