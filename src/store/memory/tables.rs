//! The store's tables as plain maps: what a snapshot holds and what the memory backend works on.

use std::collections::BTreeMap;

/// The same rows the SQLite tables hold, values sealed. Eviction is the engine's, as on disk:
/// it deletes kv and file rows it no longer needs and trims items; `seq` only grows.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(in crate::store) struct Tables {
    pub meta: BTreeMap<String, Vec<u8>>,
    pub kv: BTreeMap<Vec<u8>, Vec<u8>>,
    /// Keyed by `(HMAC(chat ref), id)`, so one chat's rows are one range.
    pub items: BTreeMap<(Vec<u8>, i64), Vec<u8>>,
    pub files: BTreeMap<i64, Vec<u8>>,
    pub seq: BTreeMap<String, i64>,
    /// The highest item id ever given out, deleted rows included (SQLite's AUTOINCREMENT).
    pub last_item: i64,
}
