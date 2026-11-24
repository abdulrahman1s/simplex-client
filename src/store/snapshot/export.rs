//! Saving and opening whole snapshots: a tag under the store key catches any change to the bytes.

use subtle::ConstantTimeEq;

use super::codec::{TAG, decode, encode};
use super::delta::Link;
use crate::store::StoreError;
use crate::store::handle::Store;
use crate::store::memory::Memory;
use crate::store::open::Tagged;

/// The lookup-key domain of the snapshot tag, apart from the `kv` and `chat` pseudonyms.
const TAG_DOMAIN: &str = "snapshot";

impl Store {
    /// Every row of the store, sealed, and a tag over all of it; either backend can be saved.
    /// Deltas from `export_changes` follow it from now on.
    pub fn export_snapshot(&mut self) -> Result<Vec<u8>, StoreError> {
        let mut out = match self.rows.memory() {
            // Encoded where the rows lie, rather than from a copy of every row.
            Some(m) => {
                m.changed.clear();
                encode(&m.t)
            }
            None => encode(&self.rows.dump()?),
        };
        let tag = self.sealer.lookup(TAG_DOMAIN, &out);
        out.extend_from_slice(&tag);
        self.chain = Some(Link {
            base: tag,
            prev: tag,
        });
        Ok(out)
    }

    /// A memory store from `export_snapshot` bytes. A key that does not open the check row is
    /// `WrongKey`; bytes that are not a whole snapshot, or were changed, are `Corrupt`.
    pub fn open_snapshot(bytes: &[u8], key: &str) -> Result<Store, StoreError> {
        let d = decode(bytes)?;
        let tagged = if d.tag.is_some() {
            Tagged::Yes
        } else {
            Tagged::No
        };
        let mut store = Store::with_rows(Box::new(Memory::new(d.tables)), key, tagged)?;
        if let Some((signed, tag)) = d.tag {
            let want = store.sealer.lookup(TAG_DOMAIN, signed);
            if !bool::from(want.as_slice().ct_eq(tag)) {
                return Err(StoreError::Corrupt("the snapshot was changed".into()));
            }
            // A first-format snapshot starts no chain: its bytes were never authenticated.
            let tag: [u8; TAG] = tag.try_into().map_err(|_| super::codec::short())?;
            store.chain = Some(Link {
                base: tag,
                prev: tag,
            });
        }
        Ok(store)
    }
}
