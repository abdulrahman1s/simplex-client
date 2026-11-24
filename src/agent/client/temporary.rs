//! Which SMP failures are worth another attempt, and which mean a relay cannot be used this way (Agent/Client.hs `temporaryOrHostError`, `serverHostError`).
use crate::smp::proxy::BrokerError;
use crate::smp::{ErrorType, HandshakeError, ProxyClientError, ProxyError, SmpError};

/// The network or the session failed, so the same command may succeed later; a refusal or a bad command will not.
#[must_use]
pub(crate) fn is_temporary(e: &SmpError) -> bool {
    match e {
        SmpError::Io(_)
        | SmpError::Timeout
        | SmpError::NotConnected
        | SmpError::Unresponsive
        | SmpError::Closed
        | SmpError::Busy
        | SmpError::TooManyPending
        | SmpError::Handshake(_)
        | SmpError::Decode(_) => true,
        SmpError::Server(ErrorType::Proxy(ProxyError::Broker(b))) => {
            b.is_temporary() || b.is_host()
        }
        SmpError::Proxy { err, .. } => proxy_failure(err, |b| b.is_temporary() || b.is_host()),
        SmpError::Server(_)
        | SmpError::Encode(_)
        | SmpError::Crypto(_)
        | SmpError::Unexpected(_)
        | SmpError::LargeMsg
        | SmpError::Address(_) => false,
    }
}

/// `serverHostError`: the relay, or the proxy's path to it, does not support what was asked,
/// such as forwarding to an old relay. Only these let a proxied command go directly instead.
#[must_use]
pub(crate) fn is_host_error(e: &SmpError) -> bool {
    match e {
        SmpError::Handshake(HandshakeError::Version) => true,
        SmpError::Server(ErrorType::Proxy(ProxyError::Broker(b))) => b.is_host(),
        SmpError::Proxy {
            err: ProxyClientError::Protocol(ErrorType::Proxy(ProxyError::Broker(b))),
            ..
        } => b.is_host(),
        SmpError::Io(_)
        | SmpError::Timeout
        | SmpError::NotConnected
        | SmpError::Unresponsive
        | SmpError::Closed
        | SmpError::Busy
        | SmpError::TooManyPending
        | SmpError::Handshake(_)
        | SmpError::Decode(_)
        | SmpError::Server(_)
        | SmpError::Proxy { .. }
        | SmpError::Encode(_)
        | SmpError::Crypto(_)
        | SmpError::Unexpected(_)
        | SmpError::LargeMsg
        | SmpError::Address(_) => false,
    }
}

/// `PROXY _ _ (ProxyProtocolError (PROXY e))`: a session the proxy lost is worth asking for
/// again; a broker failure as `broker` says; anything else the proxy refused is final.
fn proxy_failure(err: &ProxyClientError, broker: impl Fn(&BrokerError) -> bool) -> bool {
    match err {
        ProxyClientError::Protocol(ErrorType::Proxy(ProxyError::NoSession)) => true,
        ProxyClientError::Protocol(ErrorType::Proxy(ProxyError::Broker(b))) => broker(b),
        ProxyClientError::Protocol(_)
        | ProxyClientError::Unexpected(_)
        | ProxyClientError::Response(_) => false,
    }
}
