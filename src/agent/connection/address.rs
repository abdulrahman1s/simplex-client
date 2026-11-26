//! Create a contact address: a contact-mode queue with its short link, made in one NEW (Agent.hs `prepareConnectionLink'`, `createConnectionForLink'`, `encryptContactLinkData`).
use crate::crypto::{Alg, KeyPair, sha256};
use crate::encoding::VersionRange;
use crate::smp::QueueIds;
use crate::store::Store;

use super::link_seal::sealed_link;
use super::queues::{contact_address, link_sender_id, new_rcv_queue};
use crate::agent::SmpQueueUri;
use crate::agent::client::{AgentClient, AgentEvent, ConnId};
use crate::agent::error::{AgentError, Result};
use crate::agent::links::{
    ConnReqUriData, ConnShortLink, ConnectionRequestUri, ContactConnType, ServiceScheme,
    ShortLinkScheme, shorten_short_link,
};
use crate::agent::records::{Batch, ConnRecord, ConnStatus, NewLink, RcvQueue};
use crate::agent::short_link::{UserContactData, UserLinkData};
use crate::agent::version::supported_client_range;

impl AgentClient {
    /// The links come in `AgentEvent::Address`. `user_data` goes into the short link (chat's `ContactShortLinkData`).
    /// Like the apps' default address it takes plain requests: no ratchet keys are published.
    pub fn create_address(
        &mut self,
        store: &mut Store,
        short: bool,
        user_data: Vec<u8>,
    ) -> Result<ConnId> {
        self.create_address_with(store, short, user_data, None)
    }

    /// `client_data` is the link's `clientData`; a group's link has one and is a `/g#` link.
    pub fn create_address_with(
        &mut self,
        store: &mut Store,
        short: bool,
        user_data: Vec<u8>,
        client_data: Option<String>,
    ) -> Result<ConnId> {
        let server = self
            .cfg
            .servers
            .first()
            .cloned()
            .ok_or(AgentError::Prohibited("no SMP server configured"))?;
        let id = ConnId(store.next_id("agent.conn")?);
        let vr = self.cfg.agent_versions;
        let mut rec = ConnRecord::new(id, ConnStatus::Addressing, vr.max(), true);
        let rq = new_rcv_queue(server)?;
        if short {
            rec.new_link = Some(address_link(&rq, vr, user_data, client_data.clone())?);
        }
        rec.client_data = client_data;
        rec.rcv = Some(rq);
        self.save(store, rec, None, Batch::default())?;
        self.drive(store, id);
        Ok(id)
    }

    pub(crate) fn on_address_created(
        &mut self,
        store: &mut Store,
        id: ConnId,
        ids: QueueIds,
    ) -> Result<()> {
        let (mut rec, _) = self.editable(id)?;
        let rq = rec.rcv.as_mut().ok_or(AgentError::Prohibited("no queue"))?;
        rq.rcv_id = ids.rcv_id;
        rq.snd_id = ids.snd_id.clone();
        rq.server_dh = Some(ids.rcv_dh_key);
        let rq = rq.clone();
        let (vr, data) = (self.cfg.agent_versions, rec.client_data.clone());
        let full = contact_uri(&rq, vr, ids.snd_id.clone(), data)?.to_uri();
        let short = match rec.new_link.take() {
            Some(l) => {
                if link_sender_id(&l.corr_id) != ids.snd_id {
                    return Err(AgentError::Link("sender id differs from the link's".into()));
                }
                let typed = rec.link_creds.as_ref().map(|c| char::from(c.contact_type));
                let link = ConnShortLink::Contact {
                    scheme: ShortLinkScheme::Server,
                    contact_type: match typed.map(ContactConnType::from_char) {
                        Some(Ok(t)) => t,
                        _ if rec.client_data.is_some() => ContactConnType::Group,
                        _ => ContactConnType::Contact,
                    },
                    server: rq.server,
                    link_key: l.link_key,
                };
                Some(shorten_short_link(&self.cfg.presets, link).to_uri())
            }
            None => None,
        };
        rec.client_data = None;
        rec.status = ConnStatus::Address;
        self.save(store, rec, None, Batch::default())?;
        self.conn_mut(id)?.sub.subscribed = true; // NEW subscribed it on this session
        self.emit(AgentEvent::Address {
            conn: id,
            full,
            short,
        });
        Ok(())
    }
}

/// `CRContactUri` of our queue; `sender_id` comes from IDS, or from the corrId for a short link.
pub(super) fn contact_uri(
    rq: &RcvQueue,
    vr: VersionRange,
    sender_id: Vec<u8>,
    client_data: Option<String>,
) -> Result<ConnectionRequestUri> {
    let data = ConnReqUriData {
        scheme: ServiceScheme::Simplex,
        agent_v_range: vr,
        smp_queues: vec![SmpQueueUri {
            client_v_range: supported_client_range(),
            address: contact_address(rq, sender_id)?,
        }],
        client_data,
    };
    Ok(ConnectionRequestUri::Contact {
        data,
        ratchet_keys: None,
    })
}

/// The data the server stores for an address: signed by a root key whose hash is the link's entity id.
fn address_link(
    rq: &RcvQueue,
    vr: VersionRange,
    user_data: Vec<u8>,
    client_data: Option<String>,
) -> Result<NewLink> {
    let root = KeyPair::generate(Alg::Ed25519)?;
    let entity_id = sha256(root.public.as_bytes()).to_vec();
    let ucd = UserContactData {
        direct: true,
        owners: Vec::new(),
        relays: Vec::new(),
        user_data: UserLinkData(user_data),
        ratchet_keys: None,
    };
    Ok(sealed_link(rq, vr, &root.private, entity_id, client_data, |_| Ok(ucd))?.0)
}
