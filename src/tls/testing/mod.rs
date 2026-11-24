//! Test doubles for TLS: an in-memory socket, certificates and a fault-injecting TLS 1.3 server.

mod app;
mod cert;
mod client_hello;
mod conn;
mod flight;
mod hello;
mod options;
mod pipe;
mod server;
mod tcp;

pub(crate) use cert::{ALWAYS, make_cert, make_cert_valid, validity};
pub(crate) use conn::ServerConn;
pub(crate) use options::{AppHook, MockOpts, MockResult};
pub(crate) use pipe::Pipe;
pub(crate) use server::spawn_server;
pub(crate) use tcp::{DRIP, dripping_server, tls_server};
