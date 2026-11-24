//! The delta byte format: the rows written since the last save, chained to the snapshot below.

// `MAGIC || version (u32 LE) || base tag || previous tag || meta || kv || items || files || seq
// || last item id || tag`. Each written row is its key, a presence byte and, when present, its
// sealed value; an absent one was deleted. `seq` and the last item id travel whole. The tag is
// the store's lookup MAC under `DOMAIN` over every byte before it.

use std::collections::BTreeMap;

use super::codec::{Reader, TAG, blob, count, short};
use crate::store::StoreError;

const MAGIC: &[u8; 8] = b"SXCDELTA";
const VERSION: u32 = 1;
pub(super) const DOMAIN: &str = "snapshot changes";

pub(in crate::store) type Row<K> = (K, Option<Vec<u8>>);

/// The written rows of each table, values sealed as they are kept.
#[derive(Debug, Default, PartialEq, Eq)]
pub(in crate::store) struct Written {
    pub meta: Vec<Row<String>>,
    pub kv: Vec<Row<Vec<u8>>>,
    pub items: Vec<Row<(Vec<u8>, i64)>>,
    pub files: Vec<Row<i64>>,
    pub seq: BTreeMap<String, i64>,
    pub last_item: i64,
}

/// Which save a delta follows: the whole snapshot its chain starts from, and the link before it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::store) struct Link {
    pub base: [u8; TAG],
    pub prev: [u8; TAG],
}

fn value(out: &mut Vec<u8>, v: Option<&Vec<u8>>) {
    match v {
        Some(v) => {
            out.push(1);
            blob(out, v);
        }
        None => out.push(0),
    }
}

/// Everything but the tag, which the caller appends.
pub(super) fn encode(link: Link, r: &Written) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&link.base);
    out.extend_from_slice(&link.prev);
    count(&mut out, r.meta.len());
    for (k, v) in &r.meta {
        blob(&mut out, k.as_bytes());
        value(&mut out, v.as_ref());
    }
    count(&mut out, r.kv.len());
    for (k, v) in &r.kv {
        blob(&mut out, k);
        value(&mut out, v.as_ref());
    }
    count(&mut out, r.items.len());
    for ((chat, id), v) in &r.items {
        blob(&mut out, chat);
        out.extend_from_slice(&id.to_le_bytes());
        value(&mut out, v.as_ref());
    }
    count(&mut out, r.files.len());
    for (id, v) in &r.files {
        out.extend_from_slice(&id.to_le_bytes());
        value(&mut out, v.as_ref());
    }
    count(&mut out, r.seq.len());
    for (name, next) in &r.seq {
        blob(&mut out, name.as_bytes());
        out.extend_from_slice(&next.to_le_bytes());
    }
    out.extend_from_slice(&r.last_item.to_le_bytes());
    out
}

/// A delta's link, its rows, the bytes its tag covers and the tag.
pub(super) struct Decoded<'a> {
    pub link: Link,
    pub rows: Written,
    pub signed: &'a [u8],
    pub tag: &'a [u8],
}

fn tag_of(r: &mut Reader<'_>) -> Result<[u8; TAG], StoreError> {
    r.take(TAG)?.try_into().map_err(|_| short())
}

impl Reader<'_> {
    fn value(&mut self) -> Result<Option<Vec<u8>>, StoreError> {
        match self.take(1)? {
            [0] => Ok(None),
            [1] => Ok(Some(self.blob()?)),
            _ => Err(StoreError::Corrupt(
                "a delta row is neither kept nor deleted".into(),
            )),
        }
    }
}

/// Reads front to back; anything malformed is `Corrupt`, never a panic.
pub(super) fn decode(bytes: &[u8]) -> Result<Decoded<'_>, StoreError> {
    let split = bytes.len().checked_sub(TAG).ok_or_else(short)?;
    let (signed, tag) = bytes.split_at_checked(split).ok_or_else(short)?;
    let mut r = Reader(signed);
    if r.take(MAGIC.len()).ok() != Some(MAGIC.as_slice()) {
        return Err(StoreError::Corrupt("not saved store changes".into()));
    }
    if r.u32()? != VERSION {
        return Err(StoreError::Corrupt(
            "the changes were written by another version".into(),
        ));
    }
    let link = Link {
        base: tag_of(&mut r)?,
        prev: tag_of(&mut r)?,
    };
    let rows = Written {
        meta: r.rows(|r| Ok((r.text()?, r.value()?)))?,
        kv: r.rows(|r| Ok((r.blob()?, r.value()?)))?,
        items: r.rows(|r| Ok(((r.blob()?, r.i64()?), r.value()?)))?,
        files: r.rows(|r| Ok((r.i64()?, r.value()?)))?,
        seq: r.rows(|r| Ok((r.text()?, r.i64()?)))?.into_iter().collect(),
        last_item: r.i64()?,
    };
    if !r.0.is_empty() {
        return Err(StoreError::Corrupt("bytes after the changes".into()));
    }
    Ok(Decoded {
        link,
        rows,
        signed,
        tag,
    })
}
