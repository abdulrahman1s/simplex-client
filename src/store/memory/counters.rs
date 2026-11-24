//! The memory backend's counters, with checked arithmetic: a snapshot may hold any value.

use super::Memory;
use super::journal::{Undo, swap};
use crate::store::StoreError;

fn at_max() -> StoreError {
    StoreError::Corrupt("a counter is at its maximum".into())
}

impl Memory {
    pub(super) fn new_item(&mut self, chat: &[u8]) -> Result<i64, StoreError> {
        let id = self.t.last_item.checked_add(1).ok_or_else(at_max)?;
        // Not journaled: a rolled-back id stays used, which only leaves a gap.
        self.t.last_item = id;
        let key = (chat.to_vec(), id);
        let old = swap(&mut self.t.items, key.clone(), Some(Vec::new()));
        self.note(|| Undo::Item(key, old));
        Ok(id)
    }

    pub(super) fn next_in(&mut self, name: &str) -> Result<i64, StoreError> {
        let old = self.t.seq.get(name).copied();
        let next = old.unwrap_or(0).checked_add(1).ok_or_else(at_max)?;
        self.t.seq.insert(name.to_owned(), next);
        self.note(|| Undo::Seq(name.to_owned(), old));
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::super::Tables;
    use super::*;
    use crate::store::rows::Rows;

    #[test]
    fn counters_at_their_maximum_are_corrupt_not_a_panic_or_a_wrap() {
        let mut t = Tables {
            last_item: i64::MAX,
            ..Tables::default()
        };
        t.seq.insert("friends".into(), i64::MAX);
        let mut m = Memory::new(t);
        assert!(matches!(m.item_new(b"c"), Err(StoreError::Corrupt(_))));
        assert!(matches!(m.seq_next("friends"), Err(StoreError::Corrupt(_))));
        assert_eq!(m.t.last_item, i64::MAX);
        assert!(m.t.items.is_empty());
    }
}
