//! An agent that records what the engine asked and hands back the events a test queues.

use crate::agent::links::ConnectionLink;
use crate::engine::agent_port::{AgentEvent, AgentFailure, AgentMsgId, AgentPort, ConnId};
use crate::store::Store;
use crate::types::{ProxyFallback, ProxyMode};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Call {
    CreateInvitation,
    CreateAddress {
        short: bool,
        user_data: Vec<u8>,
    },
    CreateGroupAddress {
        user_data: Vec<u8>,
        client_data: String,
    },
    AcceptRequest {
        invitation: Vec<u8>,
        conn_info: Vec<u8>,
    },
    RejectRequest {
        invitation: Vec<u8>,
    },
    Join {
        link: String,
        conn_info: Vec<u8>,
    },
    Allow {
        conn: ConnId,
        conn_info: Vec<u8>,
    },
    Send {
        conn: ConnId,
        body: Vec<u8>,
    },
    Ack {
        conn: ConnId,
        msg_id: AgentMsgId,
        receipt: Option<Vec<u8>>,
    },
    Delete {
        conn: ConnId,
    },
    SubscribeAll,
    SynchronizeRatchet {
        conn: ConnId,
        force: bool,
    },
    SetDisabledRelays(Vec<String>),
}

#[derive(Debug, Default)]
pub struct FakeAgent {
    pub calls: Vec<Call>,
    pub queued: Vec<AgentEvent>,
    /// The next call that can fail fails with this.
    pub fail_next: Option<AgentFailure>,
    /// Events `subscribe_all` queues.
    pub on_subscribe: Vec<AgentEvent>,
    /// Events `create_invitation` queues, one per call.
    pub on_invitation: Vec<AgentEvent>,
    /// What `ratchet_ad` answers.
    pub ratchet_ad: Vec<u8>,
    /// The PQ flag of each `join` and `accept_request`, in order.
    pub pq_seen: Vec<(&'static str, bool)>,
    /// The PQ flag of each `send`, in order.
    pub pq_sent: Vec<(ConnId, bool)>,
    /// The connections deleted once their sends are delivered (`delete` with `wait`).
    pub waited: Vec<ConnId>,
    /// Connections whose ratchet negotiated no PQ KEM; the rest have it both ways.
    pub classical: Vec<ConnId>,
    /// The last routing settings given.
    pub routing: Option<(ProxyMode, ProxyFallback)>,
    conns: i64,
    msgs: AgentMsgId,
}

impl FakeAgent {
    fn conn(&mut self) -> Result<ConnId, AgentFailure> {
        self.fail_next.take().map_or(Ok(()), Err)?;
        self.conns += 1;
        Ok(ConnId(self.conns))
    }

    pub fn sent(&self) -> Vec<(ConnId, Vec<u8>)> {
        self.calls
            .iter()
            .filter_map(|c| {
                if let Call::Send { conn, body } = c {
                    Some((*conn, body.clone()))
                } else {
                    None
                }
            })
            .collect()
    }
}

impl AgentPort for FakeAgent {
    type Smp = AgentEvent;

    fn create_invitation(
        &mut self,
        _: &mut Store,
        _: bool,
        _: bool,
        _: Vec<u8>,
    ) -> Result<ConnId, AgentFailure> {
        self.calls.push(Call::CreateInvitation);
        let conn = self.conn()?;
        if !self.on_invitation.is_empty() {
            self.queued.push(self.on_invitation.remove(0));
        }
        Ok(conn)
    }

    fn create_address(
        &mut self,
        _: &mut Store,
        short: bool,
        user_data: Vec<u8>,
    ) -> Result<ConnId, AgentFailure> {
        self.calls.push(Call::CreateAddress { short, user_data });
        self.conn()
    }

    fn create_group_address(
        &mut self,
        _: &mut Store,
        user_data: Vec<u8>,
        client_data: String,
    ) -> Result<ConnId, AgentFailure> {
        self.calls.push(Call::CreateGroupAddress {
            user_data,
            client_data,
        });
        self.conn()
    }

    fn accept_request(
        &mut self,
        _: &mut Store,
        invitation: &[u8],
        conn_info: Vec<u8>,
        pq: bool,
    ) -> Result<ConnId, AgentFailure> {
        self.pq_seen.push(("accept", pq));
        self.calls.push(Call::AcceptRequest {
            invitation: invitation.to_vec(),
            conn_info,
        });
        self.conn()
    }

    fn reject_request(&mut self, _: &mut Store, invitation: &[u8]) -> Result<(), AgentFailure> {
        self.calls.push(Call::RejectRequest {
            invitation: invitation.to_vec(),
        });
        self.fail_next.take().map_or(Ok(()), Err)
    }

    fn join(
        &mut self,
        _: &mut Store,
        link: &ConnectionLink,
        conn_info: Vec<u8>,
        pq: bool,
    ) -> Result<ConnId, AgentFailure> {
        self.pq_seen.push(("join", pq));
        self.calls.push(Call::Join {
            link: link.to_uri(),
            conn_info,
        });
        self.conn()
    }

    fn allow(
        &mut self,
        _: &mut Store,
        conn: &ConnId,
        conn_info: Vec<u8>,
    ) -> Result<(), AgentFailure> {
        self.calls.push(Call::Allow {
            conn: *conn,
            conn_info,
        });
        Ok(())
    }

    fn send(
        &mut self,
        _: &mut Store,
        conn: &ConnId,
        pq: bool,
        body: Vec<u8>,
    ) -> Result<AgentMsgId, AgentFailure> {
        self.calls.push(Call::Send { conn: *conn, body });
        self.pq_sent.push((*conn, pq));
        self.fail_next.take().map_or(Ok(()), Err)?;
        self.msgs += 1;
        Ok(self.msgs)
    }

    fn ack(
        &mut self,
        _: &mut Store,
        conn: &ConnId,
        msg_id: AgentMsgId,
        receipt: Option<Vec<u8>>,
    ) -> Result<(), AgentFailure> {
        self.calls.push(Call::Ack {
            conn: *conn,
            msg_id,
            receipt,
        });
        Ok(())
    }

    fn delete(&mut self, _: &mut Store, conn: &ConnId, wait: bool) -> Result<(), AgentFailure> {
        self.calls.push(Call::Delete { conn: *conn });
        if wait {
            self.waited.push(*conn);
        }
        Ok(())
    }

    fn subscribe_all(&mut self, _: &mut Store) -> Result<(), AgentFailure> {
        self.calls.push(Call::SubscribeAll);
        self.queued.append(&mut self.on_subscribe);
        Ok(())
    }

    /// The test's SMP "events" are agent events, queued as the real agent would.
    fn on_smp(&mut self, _: &mut Store, event: AgentEvent) {
        self.queued.push(event);
    }

    fn on_tick(&mut self, _: &mut Store, _: i64) {}

    fn next_wake(&self) -> Option<i64> {
        None
    }

    fn close(&mut self) {}

    fn pq_on(&self, conn: &ConnId) -> bool {
        !self.classical.contains(conn)
    }

    fn ratchet_ad(&mut self, _: &mut Store, _: &ConnId) -> Result<Vec<u8>, AgentFailure> {
        self.fail_next.take().map_or(Ok(()), Err)?;
        Ok(self.ratchet_ad.clone())
    }

    fn synchronize_ratchet(
        &mut self,
        _: &mut Store,
        conn: &ConnId,
        force: bool,
    ) -> Result<(), AgentFailure> {
        self.calls
            .push(Call::SynchronizeRatchet { conn: *conn, force });
        self.fail_next.take().map_or(Ok(()), Err)
    }

    fn relay_tester(&self) -> Option<Box<dyn crate::engine::relays::RelayTester>> {
        Some(Box::new(super::relays_pass::Answering))
    }

    fn set_disabled_relays(&mut self, disabled: &[String]) {
        self.calls.push(Call::SetDisabledRelays(disabled.to_vec()));
    }

    fn set_routing(&mut self, mode: ProxyMode, fallback: ProxyFallback) {
        self.routing = Some((mode, fallback));
    }

    fn drain_events(&mut self) -> Vec<AgentEvent> {
        std::mem::take(&mut self.queued)
    }
}
