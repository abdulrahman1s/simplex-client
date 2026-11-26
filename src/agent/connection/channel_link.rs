//! A channel's link: an address made under the channel's root key and id, whose user data (owners, relays, profile) the owner sets again later with LSET (Agent.hs `prepareConnectionLink'`, `createConnectionForLink'`, `setConnShortLink'`).
use crate::crypto::PrivateKey;
use crate::smp::{Command, QueueLinkData, Request, SmpServer};
use crate::store::Store;

use super::link_seal::{sealed_link, sealed_user_data};
use super::queues::new_rcv_queue;
use crate::agent::client::{AgentClient, AgentEvent, ConnId};
use crate::agent::error::{AgentError, Result};
use crate::agent::links::{ConnShortLink, ContactConnType, ShortLinkScheme, shorten_short_link};
use crate::agent::records::{Batch, ConnRecord, ConnStatus, LinkCreds};
use crate::agent::short_link::{
    OwnerAuth, UserContactData, UserLinkData, contact_short_link_kdf, validate_link_owners,
};

/// `UserContactData` of a channel's link: not joined directly, only through its relays.
fn channel_data(
    owners: Vec<OwnerAuth>,
    relays: Vec<ConnShortLink>,
    user_data: Vec<u8>,
) -> UserContactData {
    UserContactData {
        direct: false,
        owners,
        relays,
        user_data: UserLinkData(user_data),
        ratchet_keys: None,
    }
}

impl AgentClient {
    /// The links come in `AgentEvent::Address`, the short one a `/c#` link. `root` is the
    /// channel's root key and `entity_id` its id; `user_data` gets the short link, which the
    /// channel's profile names, and the data starts with no relays.
    pub fn create_channel_link(
        &mut self,
        store: &mut Store,
        root: &PrivateKey,
        entity_id: Vec<u8>,
        owners: Vec<OwnerAuth>,
        user_data: impl FnOnce(&str) -> Vec<u8>,
        client_data: String,
    ) -> Result<ConnId> {
        validate_link_owners(&root.public()?, &owners)?;
        let server = (self.cfg.servers.first().cloned())
            .ok_or(AgentError::Prohibited("no SMP server configured"))?;
        let id = ConnId(store.next_id("agent.conn")?);
        let vr = self.cfg.agent_versions;
        let mut rec = ConnRecord::new(id, ConnStatus::Addressing, vr.max(), false);
        let rq = new_rcv_queue(server)?;
        let (presets, server) = (self.cfg.presets.clone(), rq.server.clone());
        let ucd = |link_key: &[u8]| {
            let link = ConnShortLink::Contact {
                scheme: ShortLinkScheme::Server,
                contact_type: ContactConnType::Channel,
                server,
                link_key: link_key.to_vec(),
            };
            let uri = shorten_short_link(&presets, link).to_uri();
            Ok(channel_data(owners, Vec::new(), user_data(&uri)))
        };
        let (link, fixed) = sealed_link(&rq, vr, root, entity_id, Some(client_data.clone()), ucd)?;
        let (link_id, _) = contact_short_link_kdf(&link.link_key)?;
        rec.link_creds = Some(LinkCreds {
            link_id,
            link_key: link.link_key.clone(),
            sign_key: root.clone(),
            fixed,
            contact_type: ContactConnType::Channel.char() as u8,
        });
        rec.new_link = Some(link);
        rec.client_data = Some(client_data);
        rec.rcv = Some(rq);
        self.save(store, rec, None, Batch::default())?;
        self.drive(store, id);
        Ok(id)
    }

    /// `setConnShortLink'`: new owners, relays and user data for a link from
    /// `create_channel_link`; `AgentEvent::LinkUpdated` once the server stored them.
    pub fn set_link_data(
        &mut self,
        store: &mut Store,
        conn: ConnId,
        owners: Vec<OwnerAuth>,
        relays: Vec<ConnShortLink>,
        user_data: Vec<u8>,
    ) -> Result<()> {
        let (mut rec, ratchet) = self.editable(conn)?;
        let creds =
            (rec.link_creds.as_ref()).ok_or(AgentError::Prohibited("not a channel's link"))?;
        validate_link_owners(&creds.sign_key.public()?, &owners)?;
        let ucd = channel_data(owners, relays, user_data);
        let vr = self.cfg.agent_versions;
        rec.link_update = Some(sealed_user_data(vr, &creds.sign_key, &creds.link_key, ucd)?);
        self.save(store, rec, ratchet, Batch::default())?;
        self.conn_mut(conn)?.step.retry_at = 0;
        self.drive(store, conn);
        Ok(())
    }

    /// The LSET a pending update waits for: recipient-signed, on the link's own server.
    pub(super) fn lset_request(rec: &ConnRecord) -> Option<(SmpServer, Request)> {
        let (creds, user, rq) = (
            rec.link_creds.as_ref()?,
            rec.link_update.clone()?,
            rec.rcv.as_ref()?,
        );
        let data = QueueLinkData {
            fixed: creds.fixed.clone(),
            user,
        };
        let cmd = Command::Lset {
            link_id: creds.link_id.clone(),
            data,
        };
        Some((
            rq.server.clone(),
            Request::new(cmd, rq.rcv_id.clone(), Some(rq.auth.clone())),
        ))
    }

    /// Only the data still pending is done: when `set_link_data` gave newer data while this LSET
    /// was out, that goes next and its own answer tells the caller.
    pub(super) fn on_link_updated(&mut self, store: &mut Store, id: ConnId) -> Result<()> {
        let sent = self.conn_mut(id)?.step.lset_sent.take();
        let (mut rec, ratchet) = self.editable(id)?;
        if sent.is_none() || rec.link_update != sent {
            self.drive(store, id);
            return Ok(());
        }
        rec.link_update = None;
        self.save(store, rec, ratchet, Batch::default())?;
        self.emit(AgentEvent::LinkUpdated { conn: id });
        Ok(())
    }
}
