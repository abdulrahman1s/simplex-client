//! Never-reused ids for friends, groups and members.

use super::StoreError;
use super::handle::Store;

impl Store {
    /// The next id of the counter `name`, starting at 1. Deleting rows never lowers a counter.
    pub fn next_id(&mut self, name: &str) -> Result<i64, StoreError> {
        self.atomic(|rows, _| rows.seq_next(name))
    }
}
