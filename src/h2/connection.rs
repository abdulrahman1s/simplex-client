//! A synchronous HTTP/2 client connection: `H2Client` driven over a blocking transport with read timeouts.
use std::io::{ErrorKind, Read, Write};

use super::client::H2Client;
use super::error::{H2Error, Stop};
use super::response::{Progress, Request, Response};
use super::state::Options;

pub struct Connection<T: Read + Write> {
    io: T,
    core: H2Client,
}

impl<T: Read + Write> Connection<T> {
    /// Sends the client preface and SETTINGS; the server's SETTINGS are read with the first response.
    pub fn open(io: T, opts: Options) -> Result<Self, H2Error> {
        let mut conn = Self {
            io,
            core: H2Client::open(opts),
        };
        conn.flush()?;
        Ok(conn)
    }

    fn flush(&mut self) -> Result<(), H2Error> {
        while let Some(bytes) = self.core.take_outgoing() {
            self.io.write_all(&bytes)?;
            self.io.flush()?;
        }
        Ok(())
    }

    /// The transport, for the caller's timeout and shutdown handling.
    pub fn io_mut(&mut self) -> &mut T {
        &mut self.io
    }

    #[must_use]
    pub fn is_usable(&self) -> bool {
        self.core.is_usable()
    }

    /// Sends `req` and reads the whole response, at most `max_body` bytes of body. `watch` runs after every
    /// batch sent, every read and every read timeout of the transport; an `Err` from it abandons the request.
    /// Any failure makes the connection unusable.
    pub fn request(
        &mut self,
        req: &Request<'_>,
        max_body: usize,
        watch: &mut dyn FnMut(Progress) -> Result<(), Stop>,
    ) -> Result<Response, H2Error> {
        if !self.is_usable() {
            return Err(H2Error::Closed);
        }
        let result = self.exchange(req, max_body, watch);
        if result.is_err() {
            self.core.abandon();
        }
        result
    }

    fn exchange(
        &mut self,
        req: &Request<'_>,
        max_body: usize,
        watch: &mut dyn FnMut(Progress) -> Result<(), Stop>,
    ) -> Result<Response, H2Error> {
        self.core.start(req, max_body)?;
        let mut chunk = vec![0u8; 32768];
        loop {
            let sent = match self.core.take_outgoing() {
                Some(bytes) => {
                    self.io.write_all(&bytes)?;
                    self.io.flush()?;
                    true
                }
                None => false,
            };
            watch(self.core.progress()).map_err(H2Error::Stopped)?;
            if let Some(response) = self.core.take_response() {
                self.flush()?;
                return Ok(response);
            }
            if sent {
                continue;
            }
            match self.io.read(&mut chunk) {
                Ok(0) => return Err(H2Error::Closed),
                Ok(n) => self.core.feed(&chunk[..n])?,
                Err(e) if matches!(e.kind(), ErrorKind::WouldBlock | ErrorKind::TimedOut) => {}
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(e.into()),
            }
        }
    }

    /// Graceful close: GOAWAY; the caller shuts the transport down.
    pub fn close(&mut self) {
        self.core.close();
        let _ = self.flush(); // the peer may be gone already
    }
}
