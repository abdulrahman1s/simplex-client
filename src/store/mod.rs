//! The engine's sealed store: SQLite on disk, or memory with snapshots under one MAC (in SQLite, an older sealed row put back goes unnoticed).

mod counters;
mod error;
mod files;
mod handle;
mod items;
mod kdf;
mod kv;
mod memory;
mod op;
mod open;
mod rows;
mod seal;
mod seq;
mod snapshot;
#[cfg(feature = "native")]
mod sql;
#[cfg(test)]
mod tests;

pub use error::StoreError;
pub use handle::Store;
pub use op::Op;
