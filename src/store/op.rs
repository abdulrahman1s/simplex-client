//! A batch of writes applied in one transaction.

use super::StoreError;
use super::handle::Store;
use super::{files, items, kv};

/// One write. `apply` runs them in order and rolls all back if any fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Put {
        ns: &'static str,
        key: Vec<u8>,
        value: Vec<u8>,
    },
    Delete {
        ns: &'static str,
        key: Vec<u8>,
    },
    InsertItem {
        chat_ref: Vec<u8>,
        value: Vec<u8>,
    },
    UpdateItem {
        chat_ref: Vec<u8>,
        id: i64,
        value: Vec<u8>,
    },
    DeleteItem {
        chat_ref: Vec<u8>,
        id: i64,
    },
    PutFile {
        id: i64,
        value: Vec<u8>,
    },
    DeleteFile {
        id: i64,
    },
}

impl Store {
    /// Applies `ops` atomically. Returns the ids of the `InsertItem` ops, in order.
    pub fn apply(&mut self, ops: Vec<Op>) -> Result<Vec<i64>, StoreError> {
        self.atomic(|rows, s| {
            let mut ids = Vec::new();
            for op in &ops {
                match op {
                    Op::Put { ns, key, value } => kv::put(rows, s, ns, key, value)?,
                    Op::Delete { ns, key } => kv::delete(rows, s, ns, key)?,
                    Op::InsertItem { chat_ref, value } => {
                        ids.push(items::insert(rows, s, chat_ref, value)?);
                    }
                    Op::UpdateItem {
                        chat_ref,
                        id,
                        value,
                    } => items::update(rows, s, chat_ref, *id, value)?,
                    Op::DeleteItem { chat_ref, id } => items::delete(rows, s, chat_ref, *id)?,
                    Op::PutFile { id, value } => files::put(rows, s, *id, value)?,
                    Op::DeleteFile { id } => rows.file_delete(*id)?,
                }
            }
            Ok(ids)
        })
    }
}
