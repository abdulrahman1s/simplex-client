//! Never-reused ids for friends, groups and members.

use super::handle::Store;
use super::{StoreError, counters};

impl Store {
    /// The next id of the counter `name`, starting at 1. Deleting rows never lowers a counter.
    pub fn next_id(&mut self, name: &str) -> Result<i64, StoreError> {
        self.atomic(|rows, s| {
            let next = rows.seq_next(name)?;
            counters::bump_seq(rows, s, name, next)?;
            Ok(next)
        })
    }
}
