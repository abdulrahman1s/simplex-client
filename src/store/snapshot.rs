//! A store as one byte string: every sealed row in a versioned, length-prefixed format.

// `MAGIC || version (u32 LE) || meta || kv || items || files || seq || last item id`, each table
// a count and its rows, each blob a u32 LE length and its bytes. Values stay sealed, so the
// bytes hold what the SQLite file holds: namespaces, counters and ciphertext.

use super::StoreError;
use super::handle::Store;
use super::memory::{Memory, Tables};

const MAGIC: &[u8; 8] = b"SXCSTORE";
const VERSION: u32 = 1;

impl Store {
    /// Every row of the store, sealed, in the snapshot format; either backend can be saved.
    pub fn export_snapshot(&mut self) -> Result<Vec<u8>, StoreError> {
        Ok(encode(&self.rows.dump()?))
    }

    /// A memory store from `export_snapshot` bytes. A key that does not open the check row is
    /// `WrongKey`; bytes that are not a whole snapshot are `Corrupt`.
    pub fn open_snapshot(bytes: &[u8], key: &str) -> Result<Store, StoreError> {
        let tables = decode(bytes)?;
        Store::with_rows(Box::new(Memory::new(tables)), key, false)
    }
}

fn blob(out: &mut Vec<u8>, b: &[u8]) {
    out.extend_from_slice(&(b.len() as u32).to_le_bytes());
    out.extend_from_slice(b);
}

fn count(out: &mut Vec<u8>, n: usize) {
    out.extend_from_slice(&(n as u32).to_le_bytes());
}

pub(super) fn encode(t: &Tables) -> Vec<u8> {
    let mut out = MAGIC.to_vec();
    out.extend_from_slice(&VERSION.to_le_bytes());
    count(&mut out, t.meta.len());
    t.meta.iter().for_each(|(k, v)| {
        blob(&mut out, k.as_bytes());
        blob(&mut out, v);
    });
    count(&mut out, t.kv.len());
    t.kv.iter().for_each(|(k, v)| {
        blob(&mut out, k);
        blob(&mut out, v);
    });
    count(&mut out, t.items.len());
    t.items.iter().for_each(|((chat, id), v)| {
        blob(&mut out, chat);
        out.extend_from_slice(&id.to_le_bytes());
        blob(&mut out, v);
    });
    count(&mut out, t.files.len());
    t.files.iter().for_each(|(id, v)| {
        out.extend_from_slice(&id.to_le_bytes());
        blob(&mut out, v);
    });
    count(&mut out, t.seq.len());
    t.seq.iter().for_each(|(name, next)| {
        blob(&mut out, name.as_bytes());
        out.extend_from_slice(&next.to_le_bytes());
    });
    out.extend_from_slice(&t.last_item.to_le_bytes());
    out
}

/// Reads front to back; running out of bytes anywhere is `Corrupt`, never a panic.
struct Reader<'a>(&'a [u8]);

fn short() -> StoreError {
    StoreError::Corrupt("the snapshot is cut short".into())
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], StoreError> {
        let (head, rest) = self.0.split_at_checked(n).ok_or_else(short)?;
        self.0 = rest;
        Ok(head)
    }

    fn u32(&mut self) -> Result<u32, StoreError> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes(b.try_into().map_err(|_| short())?))
    }

    fn i64(&mut self) -> Result<i64, StoreError> {
        let b = self.take(8)?;
        Ok(i64::from_le_bytes(b.try_into().map_err(|_| short())?))
    }

    fn blob(&mut self) -> Result<Vec<u8>, StoreError> {
        let n = self.u32()? as usize;
        Ok(self.take(n)?.to_vec())
    }

    fn text(&mut self) -> Result<String, StoreError> {
        String::from_utf8(self.blob()?)
            .map_err(|_| StoreError::Corrupt("a snapshot name is not UTF-8".into()))
    }

    /// `n` rows of `row`; no space is reserved up front, so a false count cannot exhaust memory.
    fn rows<T>(
        &mut self,
        mut row: impl FnMut(&mut Self) -> Result<T, StoreError>,
    ) -> Result<Vec<T>, StoreError> {
        let n = self.u32()?;
        let mut out = Vec::new();
        for _ in 0..n {
            out.push(row(self)?);
        }
        Ok(out)
    }
}

pub(super) fn decode(bytes: &[u8]) -> Result<Tables, StoreError> {
    let mut r = Reader(bytes);
    if r.take(MAGIC.len()).ok() != Some(MAGIC.as_slice()) {
        return Err(StoreError::Corrupt("not a store snapshot".into()));
    }
    if r.u32()? != VERSION {
        return Err(StoreError::Corrupt(
            "the snapshot was written by another version".into(),
        ));
    }
    let meta = r.rows(|r| Ok((r.text()?, r.blob()?)))?;
    let kv = r.rows(|r| Ok((r.blob()?, r.blob()?)))?;
    let items = r.rows(|r| Ok(((r.blob()?, r.i64()?), r.blob()?)))?;
    let files = r.rows(|r| Ok((r.i64()?, r.blob()?)))?;
    let seq = r.rows(|r| Ok((r.text()?, r.i64()?)))?;
    let last_item = r.i64()?;
    if !r.0.is_empty() {
        return Err(StoreError::Corrupt("bytes after the snapshot".into()));
    }
    Ok(Tables {
        meta: meta.into_iter().collect(),
        kv: kv.into_iter().collect(),
        items: items.into_iter().collect(),
        files: files.into_iter().collect(),
        seq: seq.into_iter().collect(),
        last_item,
    })
}
