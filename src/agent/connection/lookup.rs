//! Reading a contact link's data without joining it: LGET, checked, handed to the caller (Agent.hs `getConnShortLink'`).
use crate::crypto::PublicKey;
use crate::store::Store;

use crate::agent::client::{AgentClient, AgentEvent, ConnId};
use crate::agent::error::{AgentError, Result};
use crate::agent::links::{ConnShortLink, ConnectionRequestUri, restore_short_link};
use crate::agent::records::{Batch, ConnRecord, ConnStatus};
use crate::agent::short_link::{FixedLinkData, OwnerAuth, UserContactData};

/// What `lookup_link` read: the fixed and the mutable link data, and the request the link names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkLookup {
    /// The key that signed the fixed data.
    pub root_key: PublicKey,
    /// `linkEntityId`: a channel's id, or a relay link's member id.
    pub entity_id: Option<Vec<u8>>,
    pub owners: Vec<OwnerAuth>,
    pub relays: Vec<ConnShortLink>,
    /// The chat layer's link data (`ContactShortLinkData`, `GroupShortLinkData`, …).
    pub user_data: Vec<u8>,
    /// The address request, already on the link's server, ready for `join`.
    pub conn_req: ConnectionRequestUri,
}

impl LinkLookup {
    pub(crate) fn new(
        fixed: &FixedLinkData,
        ucd: UserContactData,
        conn_req: ConnectionRequestUri,
    ) -> Self {
        Self {
            root_key: fixed.root_key.clone(),
            entity_id: fixed.link_entity_id.clone(),
            owners: ucd.owners,
            relays: ucd.relays,
            user_data: ucd.user_data.0,
            conn_req,
        }
    }
}

impl AgentClient {
    /// The answer comes as `AgentEvent::LinkData` on the returned connection, which is gone by
    /// then, or as `AgentEvent::Err`. Nothing is secured or sent to the address.
    pub fn lookup_link(&mut self, store: &mut Store, link: &ConnShortLink) -> Result<ConnId> {
        if !matches!(link, ConnShortLink::Contact { .. }) {
            return Err(AgentError::Prohibited("only an address's link is read"));
        }
        let id = ConnId(store.next_id("agent.conn")?);
        let v = self.cfg.agent_versions.max();
        // No connection info: what tells a lookup from a join when LGET answers.
        let mut rec = ConnRecord::new(id, ConnStatus::Resolving, v, false);
        rec.link = Some(restore_short_link(&self.cfg.presets, link.clone()).to_uri());
        self.save(store, rec, None, Batch::default())?;
        self.drive(store, id);
        Ok(id)
    }

    /// LGET answered a lookup: the record is forgotten once the data is out.
    pub(super) fn finish_lookup(
        &mut self,
        store: &mut Store,
        id: ConnId,
        found: LinkLookup,
    ) -> Result<()> {
        self.delete(store, id, false)?;
        self.emit(AgentEvent::LinkData {
            conn: id,
            data: Box::new(found),
        });
        Ok(())
    }
}
