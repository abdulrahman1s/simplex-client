//! A sans-I/O SMP session for transports without threads or blocking sockets, such as a browser's WebSocket tunnel (simplexmq Client.hs, minus its threads).

mod link;
mod machine;
mod outbox;
mod receive;
mod timers;

pub use machine::SmpSession;
