//! The undo journal of a transaction: each write keeps what it replaced, so a rollback puts it back.

use std::collections::BTreeMap;

use super::changed::Changed;
use super::tables::Tables;

/// One replaced row: its key and the value before, `None` when there was none.
pub(super) enum Undo {
    Meta(String, Option<Vec<u8>>),
    Kv(Vec<u8>, Option<Vec<u8>>),
    Item((Vec<u8>, i64), Option<Vec<u8>>),
    File(i64, Option<Vec<u8>>),
    Seq(String, Option<i64>),
}

/// Sets or removes `key`, returning what was there.
pub(super) fn swap<K: Ord, V>(map: &mut BTreeMap<K, V>, key: K, value: Option<V>) -> Option<V> {
    match value {
        Some(v) => map.insert(key, v),
        None => map.remove(&key),
    }
}

impl Undo {
    /// Puts the row back, and marks its key from what is there now, as the write did not happen.
    pub(super) fn revert(self, t: &mut Tables, changed: &mut Changed) {
        match self {
            Undo::Meta(k, v) => {
                changed.meta.settle(k.clone(), v.is_some());
                drop(swap(&mut t.meta, k, v));
            }
            Undo::Kv(k, v) => {
                changed.kv.settle(k.clone(), v.is_some());
                drop(swap(&mut t.kv, k, v));
            }
            Undo::Item(k, v) => {
                changed.items.settle(k.clone(), v.is_some());
                drop(swap(&mut t.items, k, v));
            }
            Undo::File(k, v) => {
                changed.files.settle(k, v.is_some());
                drop(swap(&mut t.files, k, v));
            }
            Undo::Seq(k, v) => drop(swap(&mut t.seq, k, v)),
        }
    }
}
