//! The agent's ids, errors, servers and events as the engine's port has them (Agent/Protocol.hs, AGPL-3.0-only).

use super::agent_port::{AgentEvent, AgentFailure, AgentMsgId, ConnId};
use super::log::Log;
use crate::agent::AgentError;
use crate::agent::client::{self, AgentEvent as Native};
use crate::agent::resync::crypto_sync_state;
use crate::ratchet::RatchetError;
use crate::smp::SmpServer;
use crate::types::model::DecryptError;

pub(super) const fn conn_id(c: client::ConnId) -> ConnId {
    ConnId(c.0)
}

pub(super) const fn native_conn(c: &ConnId) -> client::ConnId {
    client::ConnId(c.0)
}

pub(super) const fn native_msg(m: AgentMsgId) -> client::AgentMsgId {
    client::AgentMsgId(m)
}

/// The host the engine tracks as up or down; never the password.
pub(super) fn host(server: &SmpServer) -> String {
    server.hosts.first().cloned().unwrap_or_default()
}

pub(super) fn failure(e: &AgentError) -> AgentFailure {
    let detail = e.to_string();
    match e {
        AgentError::Smp(s) if client::is_temporary(s) => AgentFailure::Network(detail),
        AgentError::NotAvailable => AgentFailure::Network(detail),
        AgentError::Parse(_)
        | AgentError::MissingParam(_)
        | AgentError::Link(_)
        | AgentError::Decode(_) => AgentFailure::BadLink(detail),
        AgentError::Smp(_)
        | AgentError::Encode(_)
        | AgentError::Crypto(_)
        | AgentError::Store(_)
        | AgentError::Ratchet(_)
        | AgentError::Version
        | AgentError::Prohibited(_)
        | AgentError::ConnNotFound
        | AgentError::MsgNotFound
        | AgentError::Expired
        | AgentError::Unsupported(_) => AgentFailure::Other(detail),
    }
}

/// Subscriber.hs `agentMsgDecryptError`: the `A_CRYPTO` errors a message that did not decrypt
/// gives, which the agent reports once per message.
fn decrypt_error(e: &AgentError) -> Option<(DecryptError, u32)> {
    let AgentError::Ratchet(e) = e else {
        return None;
    };
    // The errors that move the ratchet-sync state are exactly `A_CRYPTO`'s.
    crypto_sync_state(e)?;
    Some(match e {
        RatchetError::HeaderKey => (DecryptError::Header, 1),
        RatchetError::TooManySkipped(n) => (DecryptError::TooManySkipped, *n),
        RatchetError::EarlierMessage(_) => (DecryptError::Earlier, 1),
        RatchetError::AesDecrypt
        | RatchetError::Crypto(_)
        | RatchetError::LargeMsg
        | RatchetError::Header(_)
        | RatchetError::State
        | RatchetError::KemState
        | RatchetError::DuplicateMessage
        | RatchetError::Kem(_)
        | RatchetError::Version
        | RatchetError::BadState(_)
        | RatchetError::Decode(_)
        | RatchetError::Encode(_) => (DecryptError::Other, 1),
    })
}

/// `None` for what the engine does not act on (logged where it says something).
pub(super) fn event(e: Native, log: &Log) -> Option<AgentEvent> {
    Some(match e {
        Native::Invitation { conn, full, short } => AgentEvent::Invitation {
            conn: conn_id(conn),
            full,
            short,
        },
        Native::Conf { conn, conn_info } => AgentEvent::Conf {
            conn: conn_id(conn),
            conn_info,
        },
        Native::Info { conn, conn_info } => AgentEvent::Info {
            conn: conn_id(conn),
            conn_info,
        },
        Native::Con { conn } => AgentEvent::Con {
            conn: conn_id(conn),
        },
        Native::Msg {
            conn,
            msg_id,
            body,
            integrity,
            broker_ts,
        } => AgentEvent::Msg {
            conn: conn_id(conn),
            msg_id: msg_id.0,
            body,
            integrity,
            broker_ts,
        },
        Native::Sent { conn, msg_id } => AgentEvent::Sent {
            conn: conn_id(conn),
            msg_id: msg_id.0,
        },
        Native::Merr { conn, msg_id, err } => AgentEvent::Merr {
            conn: conn_id(conn),
            msg_id: msg_id.0,
            err: failure(&err),
        },
        Native::Rcvd {
            conn,
            msg_id,
            receipt,
        } => AgentEvent::Rcvd {
            conn: conn_id(conn),
            msg_id: msg_id.0,
            status: receipt.status,
        },
        Native::Up { server } => AgentEvent::Up {
            server: host(&server),
        },
        Native::Down { server } => AgentEvent::Down {
            server: host(&server),
        },
        Native::Err { conn, err } => match decrypt_error(&err) {
            Some((error, count)) => AgentEvent::DecryptError {
                conn: conn_id(conn),
                error,
                count,
            },
            None => AgentEvent::Err {
                conn: conn_id(conn),
                err: failure(&err),
            },
        },
        Native::Mwarn { conn, msg_id, err } => {
            log.line(format_args!(
                "connection {} message {}: still sending: {err}",
                conn.0, msg_id.0
            ));
            return None;
        }
        Native::Qcont { conn } => {
            log.line(format_args!(
                "connection {}: the peer's queue has room",
                conn.0
            ));
            return None;
        }
        Native::End { conn } => {
            log.line(format_args!(
                "connection {}: another client took its queue",
                conn.0
            ));
            return None;
        }
        Native::RatchetSync { conn, state } => AgentEvent::RatchetSync {
            conn: conn_id(conn),
            state,
        },
        Native::Switch { conn, phase } => AgentEvent::Switch {
            conn: conn_id(conn),
            phase,
        },
        Native::Address { conn, full, short } => AgentEvent::Address {
            conn: conn_id(conn),
            full,
            short,
        },
        // Every request to an address of ours is the plain form, which assumes post-quantum support.
        Native::Req {
            conn,
            inv_id,
            conn_info,
        } => AgentEvent::Req {
            conn: conn_id(conn),
            invitation: inv_id,
            conn_info,
            pq: true,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smp::SmpError;

    #[test]
    fn a_dropped_connection_is_a_network_failure_and_a_refusal_is_not() {
        let io = AgentError::Smp(SmpError::Io("reset".into()));
        assert!(matches!(failure(&io), AgentFailure::Network(_)));
        let refused = AgentError::Smp(SmpError::LargeMsg);
        assert!(matches!(failure(&refused), AgentFailure::Other(_)));
    }

    #[test]
    fn a_proxy_that_lost_its_session_is_a_network_failure_and_one_refusing_auth_is_not() {
        use crate::smp::{ErrorType, ProxyClientError, ProxyError};
        let proxied = |e| {
            AgentError::Smp(SmpError::Proxy {
                proxy: "smp://p".into(),
                relay: "smp://r".into(),
                err: ProxyClientError::Protocol(ErrorType::Proxy(e)),
            })
        };
        let lost = proxied(ProxyError::NoSession);
        assert!(matches!(failure(&lost), AgentFailure::Network(_)));
        let auth = proxied(ProxyError::BasicAuth);
        assert!(matches!(failure(&auth), AgentFailure::Other(_)));
    }

    #[test]
    fn the_integrity_verdict_and_the_receipts_hash_status_reach_the_engine() {
        use crate::agent::client::{AgentMsgId, RcptStatus, Receipt};
        use crate::agent::{MsgErrorType, MsgIntegrity};
        let skipped = MsgIntegrity::Error(MsgErrorType::Skipped { from: 2, to: 4 });
        let msg = Native::Msg {
            conn: client::ConnId(1),
            msg_id: AgentMsgId(5),
            body: b"x".to_vec(),
            integrity: skipped,
            broker_ts: 1_795_000_000,
        };
        assert!(matches!(
            event(msg, &Log::discard()),
            Some(AgentEvent::Msg { integrity, broker_ts: 1_795_000_000, .. }) if integrity == skipped
        ));
        let rcvd = Native::Rcvd {
            conn: client::ConnId(1),
            msg_id: AgentMsgId(5),
            receipt: Receipt {
                status: RcptStatus::BadMsgHash,
                info: Vec::new(),
            },
        };
        assert!(matches!(
            event(rcvd, &Log::discard()),
            Some(AgentEvent::Rcvd {
                status: RcptStatus::BadMsgHash,
                ..
            })
        ));
    }

    #[test]
    fn a_message_that_did_not_decrypt_is_a_decryption_error_and_others_stay_errors() {
        use crate::ratchet::RatchetError as E;
        let err = |e| Native::Err {
            conn: client::ConnId(1),
            err: AgentError::Ratchet(e),
        };
        let got = |e| match event(err(e), &Log::discard()) {
            Some(AgentEvent::DecryptError { error, count, .. }) => Some((error, count)),
            _ => None,
        };
        assert_eq!(got(E::HeaderKey), Some((DecryptError::Header, 1)));
        assert_eq!(
            got(E::TooManySkipped(700)),
            Some((DecryptError::TooManySkipped, 700))
        );
        assert_eq!(got(E::EarlierMessage(3)), Some((DecryptError::Earlier, 1)));
        assert_eq!(got(E::AesDecrypt), Some((DecryptError::Other, 1)));
        assert_eq!(got(E::Version), None);
    }

    #[test]
    fn a_bad_link_is_a_bad_link_failure() {
        assert!(matches!(
            failure(&AgentError::Parse("x")),
            AgentFailure::BadLink(_)
        ));
    }

    #[test]
    fn the_host_never_carries_the_password() {
        let s: SmpServer =
            "smp://hpq7_4gGJiilmz5Rf-CswuU5kZGkm_zOIooSw6yALRg=:secret@relay.test:5223"
                .parse()
                .unwrap();
        assert_eq!(host(&s), "relay.test");
    }
}
