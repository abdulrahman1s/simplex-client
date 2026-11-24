//! The SQLite backend: the store as a file, through rusqlite.

mod db;
mod dump;
mod migrate;
mod rows;

pub(super) use db::Db;
