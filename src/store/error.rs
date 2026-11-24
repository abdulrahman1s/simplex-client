//! What the store can fail with.

use std::fmt;

#[derive(Debug)]
pub enum StoreError {
    Open(String),
    WrongKey,
    Io(std::io::Error),
    Corrupt(String),
    Sql(String),
    /// The caller passed something the store cannot keep (a bad namespace, a missing row).
    Invalid(&'static str),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Open(e) => write!(f, "cannot open the chat store: {e}"),
            StoreError::WrongKey => f.write_str("the chat store key is wrong"),
            StoreError::Io(e) => write!(f, "chat store I/O: {e}"),
            StoreError::Corrupt(e) => write!(f, "the chat store is damaged: {e}"),
            StoreError::Sql(e) => write!(f, "chat store query failed: {e}"),
            StoreError::Invalid(e) => write!(f, "invalid chat store request: {e}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        StoreError::Io(e)
    }
}

#[cfg(feature = "native")]
impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        match e {
            // SQLITE_CORRUPT and SQLITE_NOTADB: the file is damaged or not a store.
            rusqlite::Error::SqliteFailure(f, msg) if matches!(f.extended_code & 0xff, 11 | 26) => {
                StoreError::Corrupt(msg.unwrap_or_else(|| f.to_string()))
            }
            other => StoreError::Sql(other.to_string()),
        }
    }
}
