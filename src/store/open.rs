//! Opening a store: the KDF record gives the sealer, the check row proves the key, the marks hold.

// A store made before the marks gets them here, in one transaction that also reseals its check
// row as `CHECK_WHOLE`. Older versions of this crate only decrypt the check row, so they still
// open it.

use super::handle::Store;
use super::kdf::{KDF_ROW, Kdf};
use super::rows::Rows;
use super::{StoreError, counters};

const CHECK_AAD: &[u8] = b"meta";
const CHECK_KEY: &str = "check";
// Kept as is for compatibility with existing stores.
const CHECK_PLAIN: &[u8] = b"zaun-chat store ok";
/// The check row of a store with sealed counter marks, whose snapshots always carry a MAC.
const CHECK_WHOLE: &[u8] = b"simplex-client store ok v2";

/// Where the rows came from, and whether those bytes were authenticated as a whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tagged {
    /// A SQLite file, or a new memory store: create the store when it has no check row.
    NotASnapshot,
    /// A snapshot whose MAC the caller checks once the sealer exists.
    Yes,
    /// A first-format snapshot: no MAC, which only a store from before the marks may have.
    No,
}

impl Store {
    pub(super) fn with_rows(
        mut rows: Box<dyn Rows>,
        key: &str,
        tagged: Tagged,
    ) -> Result<Store, StoreError> {
        let check = rows.meta_get(CHECK_KEY)?;
        let kdf = match (&check, rows.meta_get(KDF_ROW)?) {
            (None, _) if tagged != Tagged::NotASnapshot => {
                return Err(StoreError::Corrupt("no key check row".into()));
            }
            (None, _) if key.is_empty() => {
                return Err(StoreError::Invalid("a new store needs a non-empty key"));
            }
            (None, _) => Kdf::new()?,
            (Some(_), Some(record)) => Kdf::decode(&record)?,
            (Some(_), None) => Kdf::Legacy,
        };
        let mut store = Store {
            rows,
            sealer: kdf.sealer(key)?,
            rev: 0,
            chain: None,
        };
        match check {
            Some(sealed) => store.check(&sealed, tagged)?,
            None => store.create(&kdf)?,
        }
        store.rev = 0;
        Ok(store)
    }

    fn create(&mut self, kdf: &Kdf) -> Result<(), StoreError> {
        let record = kdf.encode();
        self.atomic(|rows, s| {
            if let Some(record) = record {
                rows.meta_put(KDF_ROW, record)?;
            }
            rows.meta_put(
                CHECK_KEY,
                s.seal(&[CHECK_AAD, CHECK_KEY.as_bytes()], CHECK_WHOLE)?,
            )?;
            counters::create(rows, s)
        })
    }

    fn check(&mut self, sealed: &[u8], tagged: Tagged) -> Result<(), StoreError> {
        let aad = [CHECK_AAD, CHECK_KEY.as_bytes()];
        let plain = self
            .sealer
            .open(&aad, sealed)
            .map_err(|_| StoreError::WrongKey)?;
        let whole = match plain.as_slice() {
            CHECK_WHOLE => true,
            CHECK_PLAIN => false,
            _ => return Err(StoreError::Corrupt("unknown key check row".into())),
        };
        if whole && tagged == Tagged::No {
            return Err(StoreError::Corrupt(
                "this store's snapshots carry a MAC".into(),
            ));
        }
        self.atomic(|rows, s| {
            counters::check(rows, s, whole)?;
            if !whole {
                rows.meta_put(CHECK_KEY, s.seal(&aad, CHECK_WHOLE)?)?;
            }
            Ok(())
        })
    }
}
