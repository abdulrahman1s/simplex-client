//! The blocking driver of the handshake: writes the ClientHello, feeds records from the socket, writes the closing flight.
use std::io::{Read, Write};

use super::super::error::TlsError;
use super::super::record::Inbound;
use super::machine::{Established, Handshake};

/// The settled handshake, and the bytes already read past it.
pub(in crate::tls) fn perform<S: Read + Write>(
    sock: &mut S,
    alpn: &[&str],
) -> Result<(Established, Inbound), TlsError> {
    let (mut handshake, hello) = Handshake::start(alpn)?;
    sock.write_all(&hello)?;
    sock.flush()?;
    let mut inbound = Inbound::default();
    loop {
        let record = inbound.next_record(sock)?;
        if let Some(est) = handshake.on_record(record)? {
            sock.write_all(&est.flight)?;
            sock.flush()?;
            return Ok((est, inbound));
        }
    }
}
