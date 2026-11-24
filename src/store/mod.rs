//! The engine's sealed store behind a synchronous API: SQLite on disk, or memory with snapshots (ADR-0042, ADR-0017).

mod error;
mod files;
mod handle;
mod items;
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
