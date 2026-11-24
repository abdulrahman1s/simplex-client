//! The memory backend's side of a delta: its written rows as they are now, and a delta laid in.

use std::collections::BTreeMap;

use super::Memory;
use super::changed::Touched;
use crate::store::snapshot::Written;

/// Each touched key with its value now; a key with no row now goes out as deleted.
fn now<K: Ord + Clone>(
    touched: &Touched<K>,
    rows: &BTreeMap<K, Vec<u8>>,
) -> Vec<(K, Option<Vec<u8>>)> {
    let keys = touched.0.keys();
    keys.map(|k| (k.clone(), rows.get(k).cloned())).collect()
}

/// Sets each row a delta keeps, and removes each it deleted.
fn lay<K: Ord>(rows: &mut BTreeMap<K, Vec<u8>>, written: Vec<(K, Option<Vec<u8>>)>) {
    for (k, v) in written {
        match v {
            Some(v) => drop(rows.insert(k, v)),
            None => drop(rows.remove(&k)),
        }
    }
}

impl Memory {
    /// The rows written since the last save, and the counters whole. Clears the marks.
    pub(in crate::store) fn take_written(&mut self) -> Written {
        let c = &self.changed;
        let written = Written {
            meta: now(&c.meta, &self.t.meta),
            kv: now(&c.kv, &self.t.kv),
            items: now(&c.items, &self.t.items),
            files: now(&c.files, &self.t.files),
            seq: self.t.seq.clone(),
            last_item: self.t.last_item,
        };
        self.changed.clear();
        written
    }

    /// Lays in a delta that was already authenticated. It is part of the saved chain, so
    /// nothing it sets is marked as changed.
    pub(in crate::store) fn apply(&mut self, w: Written) {
        lay(&mut self.t.meta, w.meta);
        lay(&mut self.t.kv, w.kv);
        lay(&mut self.t.items, w.items);
        lay(&mut self.t.files, w.files);
        self.t.seq = w.seq;
        self.t.last_item = w.last_item;
    }
}
