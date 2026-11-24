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

pub(crate) use cert::make_cert;
pub(crate) use conn::ServerConn;
pub(crate) use options::{AppHook, MockOpts, MockResult};
pub(crate) use pipe::Pipe;
pub(crate) use server::spawn_server;
