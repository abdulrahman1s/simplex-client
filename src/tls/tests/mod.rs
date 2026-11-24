//! TLS tests: the client against an in-process server (`testing`) and against real SMP servers.

mod boundaries;
mod client;
mod conformance;
mod der;
mod handshake;
mod identity;
mod live;
mod negative;
mod pin;
mod sticky;
mod stream;

use crate::crypto::Alg;
use crate::tls::testing::{MockOpts, MockResult, Pipe, spawn_server};
use crate::tls::{Pin, TlsError, TlsStream};
use std::thread::JoinHandle;

pub(super) fn start(opts: MockOpts, chunk: usize) -> (Pipe, JoinHandle<MockResult>) {
    let (client, server) = Pipe::pair(chunk);
    (client, spawn_server(server, opts))
}

pub(super) fn connect(client: Pipe) -> Result<TlsStream<Pipe>, TlsError> {
    TlsStream::connect(client, &["smp/1"])
}

pub(super) fn connect_pinned(client: Pipe, pin: Pin) -> Result<TlsStream<Pipe>, TlsError> {
    TlsStream::connect_pinned(client, &["smp/1"], pin)
}

pub(super) fn opts_for(key: Alg) -> MockOpts {
    MockOpts {
        key,
        echo: true,
        ..MockOpts::default()
    }
}
