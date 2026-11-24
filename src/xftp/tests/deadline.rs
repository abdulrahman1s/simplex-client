//! A dialled XFTP server that drips bytes cannot hold the connect past its timeout.
use std::time::{Duration, Instant};

use crate::encoding::b64_encode;
use crate::tls::testing::dripping_server;
use crate::xftp::{ClientConfig, XftpError, XftpServer, connect};

#[test]
fn tls_handshake_dripping_ccs_records_times_out() {
    let (port, _) = dripping_server();
    let server: XftpServer = format!("xftp://{}@127.0.0.1:{port}", b64_encode(&[1; 32], true))
        .parse()
        .unwrap();
    let cfg = ClientConfig {
        connect_timeout: Duration::from_millis(600),
        ..ClientConfig::default()
    };
    #[allow(clippy::disallowed_methods, reason = "measures a real socket")]
    let started = Instant::now();
    let r = connect(&server, &cfg);
    assert!(matches!(r, Err(XftpError::TimedOut)), "{:?}", r.err());
    assert!(started.elapsed() < Duration::from_secs(3));
}
