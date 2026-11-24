//! A sender command to someone else's relay, directly or through a proxy (Agent/Client.hs `sendOrProxySMPCommand`, `getSMPProxyClient`, `withProxySession`).
use super::state::{Forward, Relay, Waiting};
use crate::agent::client::{AgentClient, ConnId, Op, OpKind};
use crate::smp::proxy::{seal, session_request};
use crate::smp::{ProxiedRelay, Request, SmpServer};

impl AgentClient {
    /// Every command to a queue on someone else's relay comes here: SEND, SKEY, LKEY, LGET. False
    /// when it could not even be queued, as with `request`.
    pub(crate) fn request_to_peer(
        &mut self,
        conn: ConnId,
        kind: OpKind,
        dest: &SmpServer,
        request: Request,
    ) -> bool {
        if !self.should_proxy(dest) {
            return self.request(conn, kind, dest, request);
        }
        let Some(proxy) = self.proxy_for(dest) else {
            return false;
        };
        let w = Waiting {
            conn,
            kind,
            request,
            retried: false,
        };
        self.forward(proxy, dest.clone(), w)
    }

    /// Forwards through the session with `dest`, asking for one first if there is none.
    pub(super) fn forward(&mut self, proxy: SmpServer, dest: SmpServer, w: Waiting) -> bool {
        let key = (proxy, dest);
        let direct = self.direct_allowed(&key.1);
        match self.proxies.relays.get_mut(&key) {
            Some(Relay::Ready(relay)) => {
                let relay = relay.clone();
                self.send_forward(key, &relay, w)
            }
            Some(Relay::Asking(waiting)) => {
                waiting.push(w);
                true
            }
            Some(Relay::Failed) if direct => self.request(w.conn, w.kind, &key.1, w.request),
            // The reference answers with the cached failure, which the sender retries.
            Some(Relay::Failed) => false,
            None => self.ask_session(key, w),
        }
    }

    fn ask_session(&mut self, key: (SmpServer, SmpServer), w: Waiting) -> bool {
        let smp = &self.cfg.smp;
        let request = session_request(
            &key.1,
            key.0.basic_auth.clone(),
            smp.connect_timeout + smp.command_timeout,
        );
        let tag = self.ops.next_tag();
        if self.servers.send(&key.0, request, tag).is_err() {
            return false;
        }
        self.proxies.asking.insert(tag, key.clone());
        self.proxies.relays.insert(key, Relay::Asking(vec![w]));
        true
    }

    pub(super) fn send_forward(
        &mut self,
        (proxy, dest): (SmpServer, SmpServer),
        relay: &ProxiedRelay,
        w: Waiting,
    ) -> bool {
        let Ok((pfwd, opener)) = seal(relay, &w.request, self.cfg.smp.command_timeout * 2) else {
            return false;
        };
        let tag = self.ops.next_tag();
        if self.servers.send(&proxy, pfwd, tag).is_err() {
            return false;
        }
        let op = Op {
            conn: w.conn,
            kind: w.kind,
            server: proxy,
        };
        self.ops.insert(tag, op);
        let fwd = Forward {
            dest,
            session_id: relay.session_id.clone(),
            opener,
            request: w.request,
            retried: w.retried,
        };
        self.proxies.forwards.insert(tag, fwd);
        true
    }
}
