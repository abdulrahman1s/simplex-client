//! Agent and ratchet state. Row key = `namespace || 0 || HMAC(key)`; the value holds the key.
//!
//! The namespace is the only clear part, so a prefix scan lists one namespace and filters on
//! the real keys found inside the sealed values.

use super::StoreError;
use super::handle::Store;
use super::rows::Rows;
use super::seal::Sealer;

fn row_key(s: &Sealer, ns: &str, key: &[u8]) -> Result<Vec<u8>, StoreError> {
    if ns.is_empty() || ns.as_bytes().contains(&0) {
        return Err(StoreError::Invalid("bad kv namespace"));
    }
    let mut k = ns.as_bytes().to_vec();
    k.push(0);
    k.extend_from_slice(&s.lookup("kv", &[ns.as_bytes(), &[0], key].concat()));
    Ok(k)
}

fn pack(key: &[u8], value: &[u8]) -> Vec<u8> {
    let mut out = (key.len() as u32).to_le_bytes().to_vec();
    out.extend_from_slice(key);
    out.extend_from_slice(value);
    out
}

fn unpack(plain: &[u8]) -> Result<(&[u8], &[u8]), StoreError> {
    let bad = || StoreError::Corrupt("bad kv record".into());
    let (len, rest) = plain.split_first_chunk::<4>().ok_or_else(bad)?;
    rest.split_at_checked(u32::from_le_bytes(*len) as usize)
        .ok_or_else(bad)
}

pub(super) fn put(
    rows: &mut dyn Rows,
    s: &Sealer,
    ns: &str,
    key: &[u8],
    value: &[u8],
) -> Result<(), StoreError> {
    let k = row_key(s, ns, key)?;
    let sealed = s.seal(&[b"kv", &k], &pack(key, value))?;
    rows.kv_put(k, sealed)
}

pub(super) fn delete(
    rows: &mut dyn Rows,
    s: &Sealer,
    ns: &str,
    key: &[u8],
) -> Result<(), StoreError> {
    rows.kv_delete(&row_key(s, ns, key)?)
}

impl Store {
    pub fn get(&mut self, ns: &str, key: &[u8]) -> Result<Option<Vec<u8>>, StoreError> {
        let k = row_key(&self.sealer, ns, key)?;
        let Some(sealed) = self.rows.kv_get(&k)? else {
            return Ok(None);
        };
        let plain = self.sealer.open(&[b"kv", &k], &sealed)?;
        Ok(Some(unpack(&plain)?.1.to_vec()))
    }

    pub fn put(&mut self, ns: &str, key: &[u8], value: &[u8]) -> Result<(), StoreError> {
        self.atomic(|rows, s| put(rows, s, ns, key, value))
    }

    pub fn delete(&mut self, ns: &str, key: &[u8]) -> Result<(), StoreError> {
        self.atomic(|rows, s| delete(rows, s, ns, key))
    }

    /// Every key of `ns` that starts with `prefix`, in no particular order.
    pub fn keys_with_prefix(
        &mut self,
        ns: &str,
        prefix: &[u8],
    ) -> Result<Vec<Vec<u8>>, StoreError> {
        row_key(&self.sealer, ns, b"")?;
        let (lo, mut hi) = ([ns.as_bytes(), &[0]].concat(), ns.as_bytes().to_vec());
        hi.push(1);
        let mut keys = Vec::new();
        for (k, sealed) in self.rows.kv_range(&lo, &hi)? {
            let plain = self.sealer.open(&[b"kv", &k], &sealed)?;
            let key = unpack(&plain)?.0;
            if key.starts_with(prefix) {
                keys.push(key.to_vec());
            }
        }
        Ok(keys)
    }
}
