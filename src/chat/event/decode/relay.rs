//! Decoding of channel relay and roster events (Protocol.hs `appJsonToCM`, `x.grp.relay.*`, `x.grp.roster*`).
use super::Obj;
use crate::chat::error::Result;
use crate::chat::event::ChatEvent as E;
use crate::chat::ids::b64_decode;
use crate::chat::json::{opt, req};

pub(super) fn inv(o: &Obj) -> Result<E> {
    Ok(E::GrpRelayInv(req(o, "groupRelayInvitation")?))
}

/// `fromMaybe defaultRelayCapabilities <$> opt "relayCap"`.
pub(super) fn acpt(o: &Obj) -> Result<E> {
    Ok(E::GrpRelayAcpt {
        relay_link: req(o, "relayLink")?,
        relay_cap: opt(o, "relayCap")?.unwrap_or_default(),
    })
}

pub(super) fn test(o: &Obj) -> Result<E> {
    Ok(E::GrpRelayTest {
        challenge: b64_decode(&req::<String>(o, "challenge")?)?,
        signature: opt::<String>(o, "signature")?
            .map(|s| b64_decode(&s))
            .transpose()?,
    })
}

pub(super) fn new(o: &Obj) -> Result<E> {
    Ok(E::GrpRelayNew {
        relay_link: req(o, "relayLink")?,
    })
}

pub(super) fn reject(o: &Obj) -> Result<E> {
    Ok(E::GrpRelayReject {
        reason: req(o, "reason")?,
    })
}

pub(super) fn cap(o: &Obj) -> Result<E> {
    Ok(E::GrpRelayCap(req(o, "relayCap")?))
}

pub(super) fn roster(o: &Obj) -> Result<E> {
    Ok(E::GrpRoster {
        version: req(o, "version")?,
        file_inv: req(o, "fileInv")?,
    })
}

pub(super) fn roster_ack(o: &Obj) -> Result<E> {
    Ok(E::GrpRosterAck {
        version: req(o, "version")?,
        error: opt(o, "error")?,
    })
}

pub(super) fn roster_request(o: &Obj) -> Result<E> {
    Ok(E::GrpRosterRequest {
        version: opt(o, "version")?,
    })
}
