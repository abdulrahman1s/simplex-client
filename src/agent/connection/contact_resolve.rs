//! An address's link data resolved with LGET: the queue it names is who the request goes to, or, for a lookup, the caller (Agent.hs `getConnShortLink'`).
use crate::smp::{Response, SmpServer};
use crate::store::Store;

use super::lookup::LinkLookup;
use crate::agent::SmpQueueUri;
use crate::agent::client::{AgentClient, ConnId};
use crate::agent::error::{AgentError, Result};
use crate::agent::links::{BinaryConnReq, ConnShortLink, ConnectionMode, ConnectionRequestUri};
use crate::agent::records::Batch;
use crate::agent::short_link::{
    ConnLinkData, EncLinkData, contact_short_link_kdf, decrypt_link_data,
};

impl AgentClient {
    /// LGET answered with an address's link data (`getConnShortLink'`): the request is made to the queue it names.
    pub(crate) fn on_contact_resolved(
        &mut self,
        store: &mut Store,
        id: ConnId,
        r: Response,
    ) -> Result<()> {
        let Response::Lnk { sender_id, data } = r else {
            return Err(AgentError::Prohibited("LGET answered without LNK"));
        };
        let (mut rec, _) = self.editable(id)?;
        let link = ConnShortLink::parse(rec.link.as_deref().unwrap_or_default())?;
        let ConnShortLink::Contact {
            server, link_key, ..
        } = link
        else {
            return Err(AgentError::Link("not an address".into()));
        };
        let (_, key) = contact_short_link_kdf(&link_key)?;
        let enc = EncLinkData {
            fixed: data.fixed,
            user: data.user,
        };
        let (fixed, user) = decrypt_link_data(ConnectionMode::Contact, &link_key, &key, &enc)?;
        let fixed_data = fixed.clone();
        let (BinaryConnReq::Contact { mut data }, ConnLinkData::Contact { data: ucd, .. }) =
            (fixed.link_conn_req, user)
        else {
            return Err(AgentError::Link("not an address".into()));
        };
        let q = data
            .smp_queues
            .first_mut()
            .ok_or(AgentError::Parse("link without queues"))?;
        link_queue(&server, &sender_id, q)?;
        let req = ConnectionRequestUri::Contact {
            data,
            ratchet_keys: ucd.ratchet_keys.clone(),
        };
        if rec.conn_info.is_none() {
            return self.finish_lookup(store, id, LinkLookup::new(&fixed_data, ucd, req));
        }
        let ratchet = self.begin_request(&mut rec, &req)?;
        rec.link = None;
        self.save(store, rec, ratchet, Batch::default())?;
        self.drive(store, id);
        Ok(())
    }
}

/// `decryptData`: the queue the link data names is on the link's server, under the sender id
/// the server answered for, and takes the link's server, as `updateConnReqServer` does.
pub(super) fn link_queue(server: &SmpServer, sender_id: &[u8], q: &mut SmpQueueUri) -> Result<()> {
    if !same_srv_host(server, &q.address.server) || q.address.sender_id != sender_id {
        return Err(AgentError::Link("different address".into()));
    }
    q.address.server = server.clone();
    Ok(())
}

/// `sameSrvHost`: the link's server is one of the hosts the link data names for the queue.
fn same_srv_host(link: &SmpServer, queue: &SmpServer) -> bool {
    link.hosts.first().is_some_and(|h| queue.hosts.contains(h))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(hosts: &str) -> SmpServer {
        let key = crate::encoding::b64_encode(&[1; 32], true);
        format!("smp://{key}@{hosts}").parse().unwrap()
    }

    #[test]
    fn the_links_first_host_must_be_one_the_queue_names() {
        let queue = server("one.test,two.test");
        assert!(same_srv_host(&server("one.test"), &queue));
        assert!(
            same_srv_host(&server("two.test"), &queue),
            "any host of the queue"
        );
        assert!(!same_srv_host(&server("other.test"), &queue));
        // The other direction is not enough: the queue's first host is in the link's list.
        assert!(!same_srv_host(
            &server("one.test,two.test"),
            &server("two.test")
        ));
    }
}
