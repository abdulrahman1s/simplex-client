//! One SQLite connection, opened with the store's pragmas and its tables brought up to date.

use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;

use super::migrate;
use crate::store::StoreError;

/// Every statement the backend runs is cached; there are about twenty distinct ones.
const STATEMENT_CACHE: usize = 32;
/// How long a write waits for another connection's lock (sqlx's default, kept).
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

pub(in crate::store) struct Db {
    pub(in crate::store) conn: Connection,
}

impl Db {
    /// Opens or creates the file and brings its tables up to date.
    pub(in crate::store) fn open(path: &Path) -> Result<Db, StoreError> {
        // Created private before SQLite makes it, so the -wal and -shm files inherit 0600.
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(path)?;
        let conn = Connection::open(path)?;
        conn.busy_timeout(BUSY_TIMEOUT)?;
        conn.set_prepared_statement_cache_capacity(STATEMENT_CACHE);
        // As simplexmq: deleted rows are overwritten, not left in free pages. foreign_keys is
        // what the sqlx connection had; no table declares one today.
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA secure_delete = ON;
             PRAGMA foreign_keys = ON;",
        )?;
        migrate::run(&conn)?;
        Ok(Db { conn })
    }

    /// Runs a statement without arguments, cached like the rest.
    pub(super) fn exec(&self, sql: &str) -> Result<(), StoreError> {
        self.conn.prepare_cached(sql)?.execute([])?;
        Ok(())
    }
}
