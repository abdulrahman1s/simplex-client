//! `Store`: a backend of sealed rows, the sealer, and the sealed check row that proves the key.

#[cfg(feature = "native")]
use std::path::Path;

use super::StoreError;
use super::memory::{Memory, Tables};
use super::rows::Rows;
use super::seal::Sealer;

const CHECK_AAD: &[u8] = b"meta";
const CHECK_KEY: &str = "check";
const CHECK_PLAIN: &[u8] = b"zaun-chat store ok";

pub struct Store {
    pub(super) rows: Box<dyn Rows>,
    pub(super) sealer: Sealer,
    /// Counts committed writes, so an owner can tell the store changed.
    rev: u64,
}

impl Store {
    /// Opens or creates the SQLite store at `path`. A key that does not open the check row is
    /// `WrongKey`. Dropping the store closes it.
    #[cfg(feature = "native")]
    pub fn open(path: &Path, key: &str) -> Result<Store, StoreError> {
        Self::with_rows(Box::new(super::sql::Db::open(path)?), key, true)
    }

    /// A new, empty store held in memory; `export_snapshot` is how it outlives the process.
    pub fn open_memory(key: &str) -> Result<Store, StoreError> {
        Self::with_rows(Box::new(Memory::new(Tables::default())), key, true)
    }

    /// `create`: write the check row when there is none, as a new store does.
    pub(super) fn with_rows(
        rows: Box<dyn Rows>,
        key: &str,
        create: bool,
    ) -> Result<Store, StoreError> {
        let mut store = Store {
            rows,
            sealer: Sealer::new(key),
            rev: 0,
        };
        let aad: [&[u8]; 2] = [CHECK_AAD, CHECK_KEY.as_bytes()];
        match store.rows.meta_get(CHECK_KEY)? {
            Some(sealed) => store
                .sealer
                .open(&aad, &sealed)
                .map(drop)
                .map_err(|_| StoreError::WrongKey)?,
            None if create => {
                let sealed = store.sealer.seal(&aad, CHECK_PLAIN)?;
                store.atomic(|rows, _| rows.meta_put(CHECK_KEY, sealed))?;
            }
            None => return Err(StoreError::Corrupt("no key check row".into())),
        }
        store.rev = 0;
        Ok(store)
    }

    /// How many writes have been committed since the store was opened.
    #[must_use]
    pub fn revision(&self) -> u64 {
        self.rev
    }

    /// `f` in one transaction; any error rolls everything back.
    pub(super) fn atomic<R>(
        &mut self,
        f: impl FnOnce(&mut dyn Rows, &Sealer) -> Result<R, StoreError>,
    ) -> Result<R, StoreError> {
        self.rows.begin()?;
        let done = f(&mut *self.rows, &self.sealer).and_then(|r| {
            self.rows.commit()?;
            Ok(r)
        });
        match done {
            Ok(r) => {
                self.rev += 1;
                Ok(r)
            }
            Err(e) => {
                self.rows.rollback();
                Err(e)
            }
        }
    }
}
