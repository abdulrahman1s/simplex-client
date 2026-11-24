//! Why a link ended, and how long that makes the pool wait before it dials again.
use std::time::Duration;

use crate::smp::error::SmpError;
use crate::smp::tunnel::TunnelError;

/// The Worker counts new tunnels per minute; after `rate_limited` waiting less is pointless.
pub(super) const RATE_LIMIT_WAIT: Duration = Duration::from_secs(60);

/// Why a link ended, which sets how soon the next dial goes.
pub(super) enum Why {
    Failed(SmpError),
    /// The tunnel refused more tunnels from this address for now.
    Limited(SmpError),
    /// The Worker's maximum lifetime: dial again at once.
    Lifetime,
}

pub(super) fn ms(d: Duration) -> u64 {
    u64::try_from(d.as_millis()).unwrap_or(u64::MAX)
}

/// An error frame from the tunnel, at the hello or later while bytes flow.
pub(super) fn refused(e: &TunnelError, notes: &mut Vec<String>) -> Why {
    let err = SmpError::Io(format!("tunnel: {e}"));
    match e {
        TunnelError::NotTls => {
            notes.push("the tunnel did not accept our TLS ClientHello (not_tls)".into());
            Why::Failed(err)
        }
        TunnelError::RateLimited | TunnelError::TooManyTunnels => Why::Limited(err),
        TunnelError::BadRequest
        | TunnelError::ForbiddenPort
        | TunnelError::ForbiddenHost
        | TunnelError::Unauthorized
        | TunnelError::ConnectFailed
        | TunnelError::Idle
        | TunnelError::Quota
        | TunnelError::Other(_)
        | TunnelError::Malformed => Why::Failed(err),
    }
}
