//! Chat items. `chat` is `HMAC(chat ref)`; each value is sealed against its chat and row id.

use super::StoreError;
use super::handle::Store;
use super::rows::Rows;
use super::seal::Sealer;

fn chat_key(s: &Sealer, chat_ref: &[u8]) -> [u8; 32] {
    s.lookup("chat", chat_ref)
}

fn aad(chat: &[u8], id: i64) -> [Vec<u8>; 3] {
    [b"items".to_vec(), chat.to_vec(), id.to_le_bytes().to_vec()]
}

fn seal_row(
    rows: &mut dyn Rows,
    s: &Sealer,
    chat: &[u8],
    id: i64,
    v: &[u8],
) -> Result<(), StoreError> {
    let a = aad(chat, id);
    let sealed = s.seal(&[&a[0], &a[1], &a[2]], v)?;
    if !rows.item_set(chat, id, sealed)? {
        return Err(StoreError::Invalid("no such item"));
    }
    Ok(())
}

pub(super) fn insert(
    rows: &mut dyn Rows,
    s: &Sealer,
    chat_ref: &[u8],
    v: &[u8],
) -> Result<i64, StoreError> {
    let chat = chat_key(s, chat_ref);
    // The id is part of the AAD, so it must exist before the value is sealed.
    let id = rows.item_new(&chat)?;
    seal_row(rows, s, &chat, id, v)?;
    Ok(id)
}

pub(super) fn update(
    rows: &mut dyn Rows,
    s: &Sealer,
    chat_ref: &[u8],
    id: i64,
    v: &[u8],
) -> Result<(), StoreError> {
    seal_row(rows, s, &chat_key(s, chat_ref), id, v)
}

pub(super) fn delete(
    rows: &mut dyn Rows,
    s: &Sealer,
    chat_ref: &[u8],
    id: i64,
) -> Result<(), StoreError> {
    rows.item_delete(&chat_key(s, chat_ref), id)
}

impl Store {
    pub fn insert_item(&mut self, chat_ref: &[u8], v: &[u8]) -> Result<i64, StoreError> {
        self.atomic(|rows, s| insert(rows, s, chat_ref, v))
    }

    pub fn update_item(&mut self, chat_ref: &[u8], id: i64, v: &[u8]) -> Result<(), StoreError> {
        self.atomic(|rows, s| update(rows, s, chat_ref, id, v))
    }

    pub fn delete_item(&mut self, chat_ref: &[u8], id: i64) -> Result<(), StoreError> {
        self.atomic(|rows, s| delete(rows, s, chat_ref, id))
    }

    /// The newest `count` items with id below `before` (all when `None`), oldest first.
    pub fn page(
        &mut self,
        chat_ref: &[u8],
        before: Option<i64>,
        count: u32,
    ) -> Result<Vec<(i64, Vec<u8>)>, StoreError> {
        let chat = chat_key(&self.sealer, chat_ref);
        let mut rows = self
            .rows
            .item_page(&chat, before.unwrap_or(i64::MAX), count)?;
        rows.reverse();
        rows.into_iter()
            .map(|(id, sealed)| {
                let a = aad(&chat, id);
                Ok((id, self.sealer.open(&[&a[0], &a[1], &a[2]], &sealed)?))
            })
            .collect()
    }
}
