//! Plain TCP under the SMP and XFTP transports; the socket seam keeps TLS testable.

mod connect;
mod deadline;
mod error;
mod socket;
mod watchdog;

pub use connect::connect_tcp;
pub use deadline::Deadline;
pub use error::NetError;
pub use socket::Socket;
pub use watchdog::Watchdog;
