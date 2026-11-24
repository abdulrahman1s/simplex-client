//! The in-memory backend: sealed rows in ordered maps, for the browser and for fast tests.

mod counters;
mod journal;
mod tables;

use journal::{Undo, swap};
pub(super) use tables::Tables;

use super::StoreError;
use super::counters::Marks;
use super::rows::{KvRow, Rows};

pub(super) struct Memory {
    pub(super) t: Tables,
    /// The open transaction's writes, newest last. Eviction: emptied by `commit` and `rollback`.
    journal: Option<Vec<Undo>>,
}

impl Memory {
    pub(super) fn new(t: Tables) -> Self {
        Self { t, journal: None }
    }

    fn note(&mut self, undo: impl FnOnce() -> Undo) {
        if let Some(j) = &mut self.journal {
            j.push(undo());
        }
    }
}

impl Rows for Memory {
    fn begin(&mut self) -> Result<(), StoreError> {
        if self.journal.is_some() {
            return Err(StoreError::Invalid("a transaction is already open"));
        }
        self.journal = Some(Vec::new());
        Ok(())
    }

    fn commit(&mut self) -> Result<(), StoreError> {
        self.journal = None;
        Ok(())
    }

    fn rollback(&mut self) {
        for undo in self.journal.take().unwrap_or_default().into_iter().rev() {
            undo.revert(&mut self.t);
        }
    }

    fn meta_get(&mut self, k: &str) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self.t.meta.get(k).cloned())
    }

    fn meta_put(&mut self, k: &str, v: Vec<u8>) -> Result<(), StoreError> {
        let old = swap(&mut self.t.meta, k.to_owned(), Some(v));
        self.note(|| Undo::Meta(k.to_owned(), old));
        Ok(())
    }

    fn kv_get(&mut self, k: &[u8]) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self.t.kv.get(k).cloned())
    }

    fn kv_put(&mut self, k: Vec<u8>, v: Vec<u8>) -> Result<(), StoreError> {
        let old = swap(&mut self.t.kv, k.clone(), Some(v));
        self.note(|| Undo::Kv(k, old));
        Ok(())
    }

    fn kv_delete(&mut self, k: &[u8]) -> Result<(), StoreError> {
        let old = swap(&mut self.t.kv, k.to_vec(), None);
        self.note(|| Undo::Kv(k.to_vec(), old));
        Ok(())
    }

    fn kv_range(&mut self, lo: &[u8], hi: &[u8]) -> Result<Vec<KvRow>, StoreError> {
        let rows = self.t.kv.range(lo.to_vec()..hi.to_vec());
        Ok(rows.map(|(k, v)| (k.clone(), v.clone())).collect())
    }

    fn item_new(&mut self, chat: &[u8]) -> Result<i64, StoreError> {
        self.new_item(chat)
    }

    fn item_set(&mut self, chat: &[u8], id: i64, v: Vec<u8>) -> Result<bool, StoreError> {
        let key = (chat.to_vec(), id);
        let Some(row) = self.t.items.get_mut(&key) else {
            return Ok(false);
        };
        let old = std::mem::replace(row, v);
        self.note(|| Undo::Item(key, Some(old)));
        Ok(true)
    }

    fn item_delete(&mut self, chat: &[u8], id: i64) -> Result<(), StoreError> {
        let key = (chat.to_vec(), id);
        let old = swap(&mut self.t.items, key.clone(), None);
        self.note(|| Undo::Item(key, old));
        Ok(())
    }

    fn item_page(
        &mut self,
        chat: &[u8],
        before: i64,
        count: u32,
    ) -> Result<Vec<(i64, Vec<u8>)>, StoreError> {
        let rows = self
            .t
            .items
            .range((chat.to_vec(), i64::MIN)..(chat.to_vec(), before));
        let newest = rows.rev().take(count as usize);
        Ok(newest.map(|((_, id), v)| (*id, v.clone())).collect())
    }

    fn file_get(&mut self, id: i64) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(self.t.files.get(&id).cloned())
    }

    fn file_put(&mut self, id: i64, v: Vec<u8>) -> Result<(), StoreError> {
        let old = swap(&mut self.t.files, id, Some(v));
        self.note(|| Undo::File(id, old));
        Ok(())
    }

    fn file_delete(&mut self, id: i64) -> Result<(), StoreError> {
        let old = swap(&mut self.t.files, id, None);
        self.note(|| Undo::File(id, old));
        Ok(())
    }

    fn seq_next(&mut self, name: &str) -> Result<i64, StoreError> {
        self.next_in(name)
    }

    fn counters(&mut self) -> Result<Marks, StoreError> {
        Ok(Marks {
            last_item: self.t.last_item,
            seq: self.t.seq.clone(),
        })
    }

    fn dump(&mut self) -> Result<Tables, StoreError> {
        Ok(self.t.clone())
    }
}
