//! A sans-I/O XFTP session for transports without threads or blocking sockets: TLS, HTTP/2 and the handshake, then one command at a time (Client.hs `getXFTPClient`, `sendXFTPCommand`, minus its threads).

mod ending;
mod machine;
mod receive;
mod send;

pub use machine::{SessionEvent, XftpSession};
