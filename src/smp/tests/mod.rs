//! SMP client tests: golden encodings from the Haskell `ServerTests.hs` syntax tests and nook's `smp.unit.test.ts`, then the connection against an in-memory server.
mod auth;
mod certs;
mod commands;
mod connection;
mod fake;
mod handshake;
mod keepalive;
mod live;
mod responses;
mod session;
mod session_keepalive;
mod session_rig;
mod support;
mod tagged;
mod tls_server;
mod transmission;
