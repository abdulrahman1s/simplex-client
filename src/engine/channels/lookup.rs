//! A link read for a channel (Subscriber.hs `CFGetRelayDataJoin`, `CFGetRelayDataAccept`; Commands.hs `getShortLinkConnReq`): each answer to the step that asked, and a failure to it.

use crate::agent::connection::LinkLookup;
use crate::engine::agent_port::{AgentFailure, AgentPort, ConnId};
use crate::engine::error::{EngineError, Result, fail};
use crate::engine::machine::Engine;
use crate::engine::records::{ConnKind, ConnRecord, Lookup};
use crate::types::RequestKind;

impl<A: AgentPort> Engine<A> {
    /// Starts reading `link` for `why`.
    pub(crate) fn look_up(&mut self, link: &str, why: Lookup) -> Result<()> {
        let parsed = crate::agent::links::ConnShortLink::parse(link)
            .map_err(|_| fail(crate::engine::error::Code::BadLink, "not a short link"))?;
        let conn = self.agent.lookup_link(&mut self.store, &parsed)?;
        self.save_conn(ConnRecord {
            conn,
            kind: ConnKind::Lookup(why),
        })?;
        Ok(())
    }

    /// The lookup's record goes; `agent`: its agent connection too, which a failed lookup
    /// still has (it would read the link again on every `Up`).
    fn take_lookup(&mut self, conn: &ConnId, agent: bool) -> Result<Option<Lookup>> {
        let Some(ConnKind::Lookup(why)) = self.state.conns.get(conn).map(|c| c.kind.clone()) else {
            return Ok(None);
        };
        if agent {
            self.forget_conn(conn)?;
        } else {
            self.store.delete(
                crate::engine::records::NS,
                &crate::engine::records::keys::conn(conn),
            )?;
            self.state.conns.remove(conn);
        }
        Ok(Some(why))
    }

    pub(crate) fn on_link_data(&mut self, conn: &ConnId, data: LinkLookup) -> Result<()> {
        let Some(why) = self.take_lookup(conn, false)? else {
            return Ok(());
        };
        let r = match &why {
            Lookup::Channel { link } => self.on_channel_link(link, data),
            Lookup::RelayJoin { group, member } => {
                self.on_relay_link_to_join(*group, *member, data)
            }
            Lookup::RelayAddress { group, member } => self.on_relay_address(*group, *member, data),
            Lookup::RelayAccept { group, member } => {
                self.on_relay_link_accepted(*group, *member, data)
            }
        };
        match r {
            Ok(()) => Ok(()),
            Err(e) => self.lookup_failed(&why, e),
        }
    }

    /// `Some` when `conn` was a lookup's.
    pub(crate) fn on_lookup_error(
        &mut self,
        conn: &ConnId,
        err: &AgentFailure,
    ) -> Option<Result<()>> {
        let why = match self.take_lookup(conn, true) {
            Ok(Some(why)) => why,
            Ok(None) => return None,
            Err(e) => return Some(Err(e)),
        };
        Some(self.lookup_failed(&why, EngineError::from(err.clone())))
    }

    /// Joining a channel is the user's request, so its failure is told; a relay's is the
    /// relay's status, and the log's.
    fn lookup_failed(&mut self, why: &Lookup, e: EngineError) -> Result<()> {
        match why {
            Lookup::Channel { .. } => self.fail_event(RequestKind::Connect, e),
            Lookup::RelayJoin { group, member }
            | Lookup::RelayAddress { group, member }
            | Lookup::RelayAccept { group, member } => {
                self.log
                    .line(format_args!("a channel relay's link failed: {}", e.detail));
                self.relay_failed(*group, *member)
            }
        }
    }
}
