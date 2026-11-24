//! What a proxy answers: a session with the destination, or a forwarded command's result (Agent/Client.hs `newProxiedRelay`, `sendViaProxy`).
use super::state::{Forward, Relay, Waiting};
use crate::agent::client::{AgentClient, ConnId, Op, OpKind, is_host_error};
use crate::smp::{
    Answer, ErrorType, ProxiedRelay, ProxyClientError, ProxyError, Request, SmpError, SmpServer,
};
use crate::store::Store;

/// A server as errors name it: never its password.
fn address(s: &SmpServer) -> String {
    let mut s = s.clone();
    s.basic_auth = None;
    s.to_string()
}

fn proxy_error(proxy: &SmpServer, dest: &SmpServer, err: ProxyClientError) -> SmpError {
    SmpError::Proxy {
        proxy: address(proxy),
        relay: address(dest),
        err,
    }
}

impl AgentClient {
    /// PRXY answered: the commands waiting go out, or each hears the failure.
    pub(crate) fn on_session_answer(
        &mut self,
        store: &mut Store,
        (proxy, dest): (SmpServer, SmpServer),
        answer: Answer,
    ) {
        let key = (proxy.clone(), dest.clone());
        let Some(Relay::Asking(waiting)) = self.proxies.relays.remove(&key) else {
            return;
        };
        let e = match ProxiedRelay::from_answer(&dest, proxy.basic_auth.clone(), answer) {
            Ok(relay) => {
                self.proxies
                    .relays
                    .insert(key.clone(), Relay::Ready(relay.clone()));
                for w in waiting {
                    let (conn, kind) = (w.conn, w.kind);
                    if !self.send_forward(key.clone(), &relay, w) {
                        self.answer_op(store, conn, kind, Err(SmpError::NotConnected));
                    }
                }
                return;
            }
            // The proxy's reports about the relay name both servers, as PFWD's do.
            Err(SmpError::Server(e @ ErrorType::Proxy(ProxyError::Broker(_)))) => {
                proxy_error(&proxy, &dest, ProxyClientError::Protocol(e))
            }
            Err(e) => e,
        };
        if is_host_error(&e) {
            self.proxies.relays.insert(key, Relay::Failed);
        } else {
            self.proxies.chosen.remove(&dest);
        }
        for w in waiting {
            self.after_proxy_failure(store, &dest, w, e.clone());
        }
    }

    fn after_proxy_failure(
        &mut self,
        store: &mut Store,
        dest: &SmpServer,
        w: Waiting,
        e: SmpError,
    ) {
        let (conn, kind) = (w.conn, w.kind);
        if let Some(answer) = self.fall_back(conn, kind, dest, w.request, e) {
            self.answer_op(store, conn, kind, answer);
        }
    }

    /// Directly when the relay cannot be reached through the proxy and the setting allows it;
    /// `None` when the command went out that way.
    fn fall_back(
        &mut self,
        conn: ConnId,
        kind: OpKind,
        dest: &SmpServer,
        request: Request,
        e: SmpError,
    ) -> Option<Answer> {
        if !(is_host_error(&e) && self.direct_allowed(dest)) {
            return Some(Err(e));
        }
        if self.request(conn, kind, dest, request) {
            return None;
        }
        Some(Err(SmpError::NotConnected))
    }

    /// PFWD answered. `None` when the command went out again, through a new session or directly,
    /// and its handler waits for that answer instead.
    pub(crate) fn on_forward_answer(
        &mut self,
        op: &Op,
        fwd: Forward,
        answer: Answer,
    ) -> Option<Answer> {
        let proxy = &op.server;
        let e = match fwd.opener.open(answer) {
            Ok(Ok(r)) => return Some(Ok(r)),
            Ok(Err(err)) => {
                if err == ProxyClientError::Protocol(ErrorType::Proxy(ProxyError::NoSession)) {
                    self.proxies.drop_session(proxy, &fwd.dest, &fwd.session_id);
                    // Asked once more through the same proxy, as interactive sends do not retry.
                    if !fwd.retried {
                        let w = Waiting {
                            conn: op.conn,
                            kind: op.kind,
                            request: fwd.request,
                            retried: true,
                        };
                        let sent = self.forward(proxy.clone(), fwd.dest, w);
                        return (!sent).then_some(Err(SmpError::NotConnected));
                    }
                }
                proxy_error(proxy, &fwd.dest, err)
            }
            Err(e) => e,
        };
        self.fall_back(op.conn, op.kind, &fwd.dest, fwd.request, e)
    }
}
