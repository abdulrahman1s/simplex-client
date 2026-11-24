//! The rows written since the last save, so a delta carries only those (`Store::export_changes`).

use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

/// Each written key once, with whether it held a row at the last save. A key that was not there
/// then and is gone again leaves, so this holds at most the rows of the last save plus the rows
/// now. Eviction: emptied by every save, whole or delta.
#[derive(Debug)]
pub(in crate::store) struct Touched<K: Ord>(pub BTreeMap<K, bool>);

impl<K: Ord> Default for Touched<K> {
    fn default() -> Self {
        Self(BTreeMap::new())
    }
}

impl<K: Ord> Touched<K> {
    /// `key` was written: `had` says whether it held a row just before, `has` whether it does now.
    pub(super) fn note(&mut self, key: K, had: bool, has: bool) {
        match self.0.entry(key) {
            Entry::Vacant(v) => {
                if had || has {
                    v.insert(had);
                }
            }
            Entry::Occupied(o) => {
                let in_last_save = *o.get();
                if !in_last_save && !has {
                    o.remove();
                }
            }
        }
    }

    /// A write to `key` was rolled back and `has` says whether it holds a row again. A key with
    /// no mark had none at the last save (one that did keeps its mark), so a row back is new.
    pub(super) fn settle(&mut self, key: K, has: bool) {
        match self.0.entry(key) {
            Entry::Vacant(v) => {
                if has {
                    v.insert(false);
                }
            }
            Entry::Occupied(o) => {
                if !*o.get() && !has {
                    o.remove();
                }
            }
        }
    }
}

/// The touched keys of each table. `seq` and the last item id are small and always travel whole.
#[derive(Debug, Default)]
pub(in crate::store) struct Changed {
    pub meta: Touched<String>,
    pub kv: Touched<Vec<u8>>,
    pub items: Touched<(Vec<u8>, i64)>,
    pub files: Touched<i64>,
}

impl Changed {
    pub(in crate::store) fn clear(&mut self) {
        *self = Self::default();
    }
}
