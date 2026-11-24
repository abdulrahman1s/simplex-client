//! The SQLite backend: the store as a file, through ormlite (ADR-0017).

mod db;
mod dump;
mod migrate;
mod rows;

pub(super) use db::Db;
