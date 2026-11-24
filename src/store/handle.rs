//! `Store`: a backend of sealed rows and the sealer, with its one way to write: `atomic`.

#[cfg(feature = "native")]
use std::path::Path;

use super::StoreError;
use super::memory::{Memory, Tables};
use super::open::Tagged;
use super::rows::Rows;
use super::seal::Sealer;

pub struct Store {
    pub(super) rows: Box<dyn Rows>,
    pub(super) sealer: Sealer,
    /// Counts committed writes, so an owner can tell the store changed.
    pub(super) rev: u64,
}

impl Store {
    /// Opens or creates the SQLite store at `path`. A key that does not open the check row is
    /// `WrongKey`. Dropping the store closes it.
    #[cfg(feature = "native")]
    pub fn open(path: &Path, key: &str) -> Result<Store, StoreError> {
        Self::with_rows(
            Box::new(super::sql::Db::open(path)?),
            key,
            Tagged::NotASnapshot,
        )
    }

    /// A new, empty store held in memory; `export_snapshot` is how it outlives the process.
    pub fn open_memory(key: &str) -> Result<Store, StoreError> {
        Self::with_rows(
            Box::new(Memory::new(Tables::default())),
            key,
            Tagged::NotASnapshot,
        )
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
