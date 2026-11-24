//! The sans-I/O XFTP session and the in-memory transfers against an in-process server: TLS, HTTP/2 and the XFTP commands.
mod deadline;
mod fake_h2;
pub(crate) mod fake_server;
pub(crate) mod fake_xftp;
mod mem;
mod rig;
mod session;
