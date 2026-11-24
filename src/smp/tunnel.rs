//! The WebSocket byte tunnel to SMP relays (the Worker's own protocol, not simplexmq): a JSON hello, a JSON reply, then binary frames of raw bytes.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TunnelError {
    #[error("the tunnel request was malformed")]
    BadRequest,
    #[error("the tunnel does not connect to this port")]
    ForbiddenPort,
    #[error("the tunnel does not connect to this host")]
    ForbiddenHost,
    #[error("the tunnel refused the token")]
    Unauthorized,
    #[error("the relay did not answer with TLS")]
    NotTls,
    #[error("the tunnel could not reach the relay")]
    ConnectFailed,
    #[error("the tunnel closed an idle connection")]
    Idle,
    #[error("too many new tunnels from this address; wait a minute")]
    RateLimited,
    #[error("too many tunnels open from this address")]
    TooManyTunnels,
    #[error("the tunnel used up its byte quota")]
    Quota,
    #[error("the tunnel failed with {0}")]
    Other(String),
    #[error("the tunnel reply is not valid")]
    Malformed,
}

#[derive(Serialize)]
struct Hello<'a> {
    host: &'a str,
    port: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<&'a str>,
}

#[derive(Deserialize)]
struct Reply {
    ok: bool,
    #[serde(default)]
    error: Option<String>,
}

/// The first text frame: `{"host":…,"port":…}` with `"token"` when there is one.
#[must_use]
pub fn tunnel_hello(host: &str, port: u16, token: Option<&str>) -> String {
    let hello = Hello { host, port, token };
    // A struct of strings and a number always serialises.
    serde_json::to_string(&hello).unwrap_or_default()
}

/// The most bytes one binary frame may carry; longer writes go out in several frames.
pub const MAX_FRAME: usize = 64 * 1024;

/// The Worker's answer to the hello: `{"ok":true}`, or `{"ok":false,"error":"<code>"}`. The
/// same error frame may also come later, while bytes flow (`not_tls`, `idle`, `quota`).
pub fn parse_tunnel_reply(text: &str) -> Result<(), TunnelError> {
    let reply: Reply = serde_json::from_str(text).map_err(|_| TunnelError::Malformed)?;
    if reply.ok {
        return Ok(());
    }
    Err(match reply.error.as_deref() {
        Some("bad_request") => TunnelError::BadRequest,
        Some("forbidden_port") => TunnelError::ForbiddenPort,
        Some("forbidden_host") => TunnelError::ForbiddenHost,
        Some("unauthorized") => TunnelError::Unauthorized,
        Some("not_tls") => TunnelError::NotTls,
        Some("connect_failed") => TunnelError::ConnectFailed,
        Some("idle") => TunnelError::Idle,
        Some("rate_limited") => TunnelError::RateLimited,
        Some("too_many_tunnels") => TunnelError::TooManyTunnels,
        Some("quota") => TunnelError::Quota,
        Some(other) => TunnelError::Other(other.to_owned()),
        None => TunnelError::Malformed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hello_names_host_and_port_and_leaves_out_a_missing_token() {
        assert_eq!(
            tunnel_hello("smp.example", 5223, None),
            r#"{"host":"smp.example","port":5223}"#
        );
    }

    #[test]
    fn hello_carries_the_token_and_escapes_strings() {
        let hello = tunnel_hello("a\"b", 443, Some("t\\k"));
        assert_eq!(hello, r#"{"host":"a\"b","port":443,"token":"t\\k"}"#);
        let v: serde_json::Value = serde_json::from_str(&hello).unwrap();
        assert_eq!(v["host"], "a\"b");
        assert_eq!(v["token"], "t\\k");
    }

    #[test]
    fn ok_reply_opens_the_tunnel() {
        assert_eq!(parse_tunnel_reply(r#"{"ok":true}"#), Ok(()));
    }

    #[test]
    fn every_worker_error_code_has_its_variant() {
        let codes = [
            ("bad_request", TunnelError::BadRequest),
            ("forbidden_port", TunnelError::ForbiddenPort),
            ("forbidden_host", TunnelError::ForbiddenHost),
            ("unauthorized", TunnelError::Unauthorized),
            ("not_tls", TunnelError::NotTls),
            ("connect_failed", TunnelError::ConnectFailed),
            ("idle", TunnelError::Idle),
            ("rate_limited", TunnelError::RateLimited),
            ("too_many_tunnels", TunnelError::TooManyTunnels),
            ("quota", TunnelError::Quota),
        ];
        for (code, want) in codes {
            let reply = format!(r#"{{"ok":false,"error":"{code}"}}"#);
            assert_eq!(parse_tunnel_reply(&reply), Err(want));
        }
    }

    #[test]
    fn an_unknown_code_is_kept_for_the_log() {
        assert_eq!(
            parse_tunnel_reply(r#"{"ok":false,"error":"teapot"}"#),
            Err(TunnelError::Other("teapot".into()))
        );
    }

    #[test]
    fn a_reply_that_is_not_the_protocol_is_malformed() {
        for text in ["", "ok", "{}", r#"{"ok":false}"#, r#"{"ok":"yes"}"#] {
            assert_eq!(
                parse_tunnel_reply(text),
                Err(TunnelError::Malformed),
                "{text}"
            );
        }
    }
}
