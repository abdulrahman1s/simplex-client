//! The agent event table: each event to its handler; a failure is logged (nobody asked for it).

use crate::types::model::Delivery;

use crate::agent::client::RcptStatus;
use crate::engine::agent_port::{AgentEvent, AgentPort};
use crate::engine::machine::Engine;
use crate::engine::records::ConnKind;

impl<A: AgentPort> Engine<A> {
    pub(crate) fn on_agent(&mut self, event: AgentEvent) {
        let r = match event {
            AgentEvent::Invitation { conn, full, short } => self.on_invitation(&conn, full, short),
            AgentEvent::Conf { conn, conn_info } => self.on_conf(&conn, &conn_info),
            AgentEvent::Info { conn, conn_info } => self.on_info(&conn, &conn_info),
            AgentEvent::Con { conn } => self.on_con(&conn),
            AgentEvent::Msg {
                conn,
                msg_id,
                body,
                integrity,
                broker_ts,
            } => {
                self.on_msg(&conn, msg_id, &body, integrity, broker_ts);
                Ok(())
            }
            AgentEvent::Sent { conn, msg_id } => self.on_delivery(&conn, msg_id, Delivery::Sent),
            AgentEvent::Rcvd {
                conn,
                msg_id,
                status,
            } => {
                let to = match status {
                    RcptStatus::Ok => Delivery::Delivered,
                    RcptStatus::BadMsgHash => Delivery::DeliveredBadHash,
                };
                self.on_delivery(&conn, msg_id, to)
            }
            AgentEvent::Merr { conn, msg_id, err } => {
                self.log.line(format_args!(
                    "a message was not delivered: {}",
                    err.detail()
                ));
                self.on_delivery(&conn, msg_id, Delivery::Failed)
            }
            AgentEvent::Up { server } => {
                self.on_host(server, true);
                Ok(())
            }
            AgentEvent::Down { server } => {
                self.on_host(server, false);
                Ok(())
            }
            AgentEvent::Err { conn, err } => self.on_conn_error(&conn, &err),
            AgentEvent::RatchetSync { conn, state } => self.on_ratchet_sync(&conn, state),
            AgentEvent::Switch { conn, phase } => self.on_switch(&conn, phase),
            e @ (AgentEvent::Address { conn, .. } | AgentEvent::Req { conn, .. }) => {
                match self.state.conns.get(&conn).map(|c| &c.kind) {
                    Some(&ConnKind::GroupLink { group }) => {
                        self.on_group_link_agent_event(group, e)
                    }
                    _ => self.on_address_event(e),
                }
            }
        };
        if let Err(e) = r {
            self.log
                .line(format_args!("an agent event failed: {}", e.detail));
        }
    }
}
