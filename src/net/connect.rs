//! TCP connect to a host:port, trying every resolved address (SMP address `host:port`).
use std::io;
use std::net::{TcpStream, ToSocketAddrs};

use super::{Deadline, NetError};

/// No SNI and no TLS here; all the addresses `host` resolves to share `deadline`.
pub fn connect_tcp(host: &str, port: u16, deadline: &Deadline<'_>) -> Result<TcpStream, NetError> {
    if host.ends_with(".onion") {
        return Err(NetError::OnionUnsupported(host.to_owned()));
    }
    let addrs = (host, port)
        .to_socket_addrs()
        .map_err(|source| NetError::Resolve {
            host: host.to_owned(),
            source,
        })?;
    let mut last = None;
    for addr in addrs {
        let Some(timeout) = deadline.left() else {
            break;
        };
        match TcpStream::connect_timeout(&addr, timeout) {
            Ok(stream) => {
                stream.set_nodelay(true).ok(); // a latency hint only
                return Ok(stream);
            }
            Err(e) => last = Some(e),
        }
    }
    if deadline.left().is_none() {
        return Err(NetError::TimedOut {
            host: host.to_owned(),
            port,
        });
    }
    Err(NetError::Connect {
        host: host.to_owned(),
        port,
        source: last.unwrap_or_else(|| io::Error::new(io::ErrorKind::NotFound, "no addresses")),
    })
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::time::Duration;

    use super::*;
    use crate::smp::SystemClock;

    fn deadline(clock: &SystemClock, secs: u64) -> Deadline<'_> {
        Deadline::new(clock, Duration::from_secs(secs))
    }

    #[test]
    fn onion_hosts_are_refused_before_resolving() {
        let r = connect_tcp("abcdefg.onion", 5223, &deadline(&SystemClock::new(), 1));
        assert!(matches!(r, Err(NetError::OnionUnsupported(_))));
    }

    #[test]
    fn connects_to_a_listening_loopback_port() {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        assert!(connect_tcp("127.0.0.1", port, &deadline(&SystemClock::new(), 2)).is_ok());
    }

    #[test]
    fn a_refused_port_reports_the_connect_error() {
        let port = {
            let l = TcpListener::bind("127.0.0.1:0").unwrap();
            l.local_addr().unwrap().port()
        };
        let r = connect_tcp("127.0.0.1", port, &deadline(&SystemClock::new(), 2));
        assert!(matches!(r, Err(NetError::Connect { .. })));
    }

    #[test]
    fn a_spent_deadline_is_reported_as_a_timeout() {
        let r = connect_tcp("127.0.0.1", 9, &deadline(&SystemClock::new(), 0));
        assert!(matches!(r, Err(NetError::TimedOut { .. })));
    }
}
