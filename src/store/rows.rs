//! The row operations both backends provide; every value passed here is already sealed.

use super::StoreError;
use super::counters::Marks;
use super::memory::{Memory, Tables};

/// A sealed row of the kv table: its key and value.
pub(super) type KvRow = (Vec<u8>, Vec<u8>);

/// `begin`/`commit`/`rollback` bracket `Store::atomic`; a write outside them stands alone.
pub(super) trait Rows: Send {
    fn begin(&mut self) -> Result<(), StoreError>;
    fn commit(&mut self) -> Result<(), StoreError>;
    fn rollback(&mut self);

    fn meta_get(&mut self, k: &str) -> Result<Option<Vec<u8>>, StoreError>;
    fn meta_put(&mut self, k: &str, v: Vec<u8>) -> Result<(), StoreError>;

    fn kv_get(&mut self, k: &[u8]) -> Result<Option<Vec<u8>>, StoreError>;
    fn kv_put(&mut self, k: Vec<u8>, v: Vec<u8>) -> Result<(), StoreError>;
    fn kv_delete(&mut self, k: &[u8]) -> Result<(), StoreError>;
    /// Every row with `lo <= k < hi`.
    fn kv_range(&mut self, lo: &[u8], hi: &[u8]) -> Result<Vec<KvRow>, StoreError>;

    /// A new row of `chat` with an empty value; its id is above every id ever given out.
    fn item_new(&mut self, chat: &[u8]) -> Result<i64, StoreError>;
    /// False when `chat` has no row `id`.
    fn item_set(&mut self, chat: &[u8], id: i64, v: Vec<u8>) -> Result<bool, StoreError>;
    fn item_delete(&mut self, chat: &[u8], id: i64) -> Result<(), StoreError>;
    /// The newest `count` rows of `chat` with an id below `before`, newest first.
    fn item_page(
        &mut self,
        chat: &[u8],
        before: i64,
        count: u32,
    ) -> Result<Vec<(i64, Vec<u8>)>, StoreError>;

    fn file_get(&mut self, id: i64) -> Result<Option<Vec<u8>>, StoreError>;
    fn file_put(&mut self, id: i64, v: Vec<u8>) -> Result<(), StoreError>;
    fn file_delete(&mut self, id: i64) -> Result<(), StoreError>;

    /// The counter's next value, starting at 1.
    fn seq_next(&mut self, name: &str) -> Result<i64, StoreError>;
    /// Every counter as kept in clear, and the highest item id given out.
    fn counters(&mut self) -> Result<Marks, StoreError>;

    /// Every row, as sealed as it is kept.
    fn dump(&mut self) -> Result<Tables, StoreError>;

    /// The memory backend, the only one that saves by snapshot and delta.
    fn memory(&mut self) -> Option<&mut Memory> {
        None
    }
}
