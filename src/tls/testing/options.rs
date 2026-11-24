//! What the mock TLS server should do wrong, and what it saw.
use super::conn::ServerConn;
use crate::crypto::{Alg, KeyPair};
use crate::tls::error::TlsError;

/// Runs after the handshake in place of the canned application phase.
pub(crate) type AppHook = Box<dyn FnOnce(&mut ServerConn) -> Result<(), TlsError> + Send>;

pub(crate) struct MockOpts {
    pub key: Alg,
    pub alpn: Option<&'static str>,
    pub bad_finished: bool,
    pub bad_sig: bool,
    pub empty_chain: bool,
    pub hrr: bool,
    pub cert_request: bool,
    pub compat_ccs: bool,
    pub coalesce: bool,
    pub fragment: usize,
    pub corrupt_handshake_record: bool,
    pub alert_in_handshake: bool,
    pub key_update: bool,
    pub send_data: Vec<u8>,
    pub corrupt_app_record: bool,
    /// Sent in its own record after `send_data`.
    pub second_data: Vec<u8>,
    pub close_notify_after_data: bool,
    pub echo: bool,
    /// A leaf key and the chain to present instead of a self-signed certificate of `key`.
    pub identity: Option<(KeyPair, Vec<Vec<u8>>)>,
    pub app: Option<AppHook>,
}

impl Default for MockOpts {
    fn default() -> Self {
        Self {
            key: Alg::Ed25519,
            alpn: Some("smp/1"),
            bad_finished: false,
            bad_sig: false,
            empty_chain: false,
            hrr: false,
            cert_request: false,
            compat_ccs: false,
            coalesce: false,
            fragment: 0,
            corrupt_handshake_record: false,
            alert_in_handshake: false,
            key_update: false,
            send_data: Vec::new(),
            corrupt_app_record: false,
            second_data: Vec::new(),
            close_notify_after_data: false,
            echo: false,
            identity: None,
            app: None,
        }
    }
}

#[derive(Default)]
pub(crate) struct MockResult {
    pub client_finished: Vec<u8>,
    pub client_cert_empty: bool,
    pub received: Vec<u8>,
    pub error: Option<String>,
}
