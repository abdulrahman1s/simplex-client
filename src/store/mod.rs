//! The engine's sealed store behind a synchronous API: SQLite on disk, or memory with snapshots.

mod error;
mod files;
mod handle;
mod items;
mod kdf;
mod kv;
mod memory;
mod op;
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
