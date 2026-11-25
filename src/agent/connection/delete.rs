//! Delete a connection: DEL our queue now, and forget every record of it once what waits to be sent on it has gone, then close servers nothing uses (Agent.hs `deleteConnectionsAsync'` with `waitDelivery`, `deleteConn`; AgentStore.hs `setConnDeleted`).
use std::collections::HashSet;

use crate::smp::SmpServer;
use crate::store::Store;

use super::queues::del_request;
use crate::agent::client::{AgentClient, AgentEvent, ConnId, OpKind};
use crate::agent::error::{AgentError, Result};
use crate::agent::records::{self, Batch};

/// `connDeleteDeliveryTimeout`: a deleted connection whose sends still wait goes after this
/// anyway (2 days).
pub const DELETE_DELIVERY_TIMEOUT_MS: i64 = 2 * 24 * 60 * 60 * 1000;

impl AgentClient {
    /// Nothing more is received on `conn`; what was sent on it before still goes, so the
    /// connection's record stays until those sends are delivered, given up or 2 days old.
    pub fn delete(&mut self, store: &mut Store, conn: ConnId) -> Result<()> {
        if self.conn(conn)?.sends.is_empty() {
            return self.delete_now(store, conn);
        }
        let (mut rec, _) = self.editable(conn)?;
        if let Some(rq) = rec.rcv.take().filter(|r| !r.rcv_id.is_empty()) {
            self.by_rcv.remove(&rq.rcv_id);
            // Best effort: a queue left behind expires on the server.
            self.request(conn, OpKind::Del, &rq.server.clone(), del_request(&rq));
        }
        let at = self.now();
        let mut b = Batch::default();
        b.deleting(conn, at);
        self.save(store, rec, None, b)?;
        self.conn_mut(conn)?.deleting = Some(at);
        Ok(())
    }

    /// Deleted and nothing left to send, or waited too long: forgotten now.
    pub(crate) fn finish_deleting(&mut self, store: &mut Store, conn: ConnId) -> Result<()> {
        let Ok(c) = self.conn(conn) else {
            return Ok(());
        };
        let overdue = |at: i64| self.now() - at >= DELETE_DELIVERY_TIMEOUT_MS;
        match c.deleting {
            Some(at) if c.sends.is_empty() || overdue(at) => self.delete_now(store, conn),
            Some(_) | None => Ok(()),
        }
    }

    /// Each deleted connection still waiting, checked against the timeout.
    pub(crate) fn finish_overdue(&mut self, store: &mut Store) {
        let deleted: Vec<ConnId> = (self.conns.iter())
            .filter(|(_, c)| c.deleting.is_some())
            .map(|(id, _)| *id)
            .collect();
        for id in deleted {
            if let Err(err) = self.finish_deleting(store, id) {
                self.emit(AgentEvent::Err { conn: id, err });
            }
        }
    }

    fn delete_now(&mut self, store: &mut Store, conn: ConnId) -> Result<()> {
        let c = self.conns.remove(&conn).ok_or(AgentError::ConnNotFound)?;
        if let Some(rq) = c.rec.rcv.as_ref().filter(|r| !r.rcv_id.is_empty()) {
            self.by_rcv.remove(&rq.rcv_id);
            // Best effort: a queue left behind expires on the server.
            self.request(conn, OpKind::Del, &rq.server.clone(), del_request(rq));
        }
        let mut b = Batch::default();
        for prefix in records::conn_prefixes(conn) {
            for k in store.keys_with_prefix(records::NS, &prefix)? {
                b.delete(k);
            }
        }
        b.apply(store)
    }

    /// Closes the connections to servers no queue, configured server or waiting command names.
    pub(crate) fn close_unused_servers(&mut self) {
        let mut used: HashSet<SmpServer> = self.cfg.servers.iter().cloned().collect();
        for c in self.conns.values() {
            used.extend(c.rec.rcv.iter().map(|q| q.server.clone()));
            used.extend(c.rec.snd.iter().map(|q| q.server.clone()));
            // A switch's new or old queue: its server must stay open between the switch's requests.
            used.extend(c.rec.switch.iter().map(|s| s.queue().server.clone()));
            used.extend(c.rec.request.iter().map(|r| r.queue.address.server.clone()));
            if let Ok((s, _)) = super::steps::short_target(&c.rec) {
                used.insert(s);
            }
        }
        used.extend(self.ops.servers());
        self.proxies.retain(|s| used.contains(s));
        used.extend(self.proxies.servers());
        self.servers.retain(|s| used.contains(s));
    }
}
