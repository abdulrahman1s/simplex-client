//! Saving and opening deltas: each names the snapshot it builds on and the save before it.

// A delta's tag covers its bytes, and its link names the tags before it, so a changed, dropped,
// repeated or reordered delta is caught. Deltas of another snapshot are skipped rather than
// refused: they come before a newer whole save, which already holds what they changed. Dropping
// the newest deltas is not caught, as an older snapshot is not.

use subtle::ConstantTimeEq;

use super::codec::{TAG, short};
use super::delta::{DOMAIN, Link, decode, encode};
use crate::store::handle::Store;
use crate::store::{StoreError, counters};

impl Store {
    /// The rows written since the last save, sealed and tagged, to keep after that save. Only a
    /// memory store that was saved whole, or opened from a tagged snapshot, has a chain to add to.
    pub fn export_changes(&mut self) -> Result<Vec<u8>, StoreError> {
        let Some(chain) = self.chain else {
            return Err(StoreError::Invalid(
                "changes need a whole snapshot saved first",
            ));
        };
        let m =
            (self.rows.memory()).ok_or(StoreError::Invalid("only a memory store saves changes"))?;
        let mut out = encode(chain, &m.take_written());
        let tag = self.sealer.lookup(DOMAIN, &out);
        out.extend_from_slice(&tag);
        self.chain = Some(Link {
            base: chain.base,
            prev: tag,
        });
        Ok(out)
    }

    /// A memory store from `export_snapshot` bytes and the deltas saved after them, oldest
    /// first. Deltas of an older snapshot are skipped; one missing or out of order is
    /// `ChangesDoNotFollow`; a changed one is `Corrupt`.
    pub fn open_snapshot_with_changes<D: AsRef<[u8]>>(
        bytes: &[u8],
        changes: &[D],
        key: &str,
    ) -> Result<Store, StoreError> {
        let mut store = Store::open_snapshot(bytes, key)?;
        let mut applied = false;
        for delta in changes {
            let d = decode(delta.as_ref())?;
            let Some(chain) = store.chain.filter(|c| c.base == d.link.base) else {
                continue;
            };
            if d.link.prev != chain.prev {
                return Err(StoreError::ChangesDoNotFollow);
            }
            let want = store.sealer.lookup(DOMAIN, d.signed);
            if !bool::from(want.as_slice().ct_eq(d.tag)) {
                return Err(StoreError::Corrupt("saved changes were altered".into()));
            }
            let m = (store.rows.memory()).ok_or(StoreError::Invalid("not a memory store"))?;
            m.apply(d.rows);
            let tag: [u8; TAG] = d.tag.try_into().map_err(|_| short())?;
            store.chain = Some(Link {
                base: chain.base,
                prev: tag,
            });
            applied = true;
        }
        if applied {
            // The marks and the clear counters came in together; they must still agree.
            store.atomic(|rows, s| counters::check(rows, s, true))?;
            store.rev = 0;
        }
        Ok(store)
    }
}
