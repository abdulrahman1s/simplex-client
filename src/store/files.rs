//! File transfer records, sealed against their id.

use super::StoreError;
use super::handle::Store;
use super::rows::Rows;
use super::seal::Sealer;

pub(super) fn put(rows: &mut dyn Rows, s: &Sealer, id: i64, v: &[u8]) -> Result<(), StoreError> {
    let sealed = s.seal(&[b"files", &id.to_le_bytes()], v)?;
    rows.file_put(id, sealed)
}

impl Store {
    pub fn put_file(&mut self, id: i64, v: &[u8]) -> Result<(), StoreError> {
        self.atomic(|rows, s| put(rows, s, id, v))
    }

    pub fn get_file(&mut self, id: i64) -> Result<Option<Vec<u8>>, StoreError> {
        let sealed = self.rows.file_get(id)?;
        sealed
            .map(|v| self.sealer.open(&[b"files", &id.to_le_bytes()], &v))
            .transpose()
    }

    pub fn delete_file(&mut self, id: i64) -> Result<(), StoreError> {
        self.atomic(|rows, _| rows.file_delete(id))
    }
}
