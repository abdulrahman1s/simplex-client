//! How the passphrase becomes the store key: Argon2id with a per-store salt kept in clear in `meta`.

// A store with no `kdf` row predates it and opens with the old unsalted HKDF (`Kdf::Legacy`).
// Such a store is not re-keyed: items are keyed by an HMAC of their chat reference, which the
// store does not keep, so moving to a new key would need every chat reference.

use argon2::{Algorithm, Argon2, Params, Version};
use zeroize::Zeroizing;

use super::StoreError;
use super::seal::Sealer;

/// The meta row that holds the KDF record, unsealed: it is read before the key exists.
pub(super) const KDF_ROW: &str = "kdf";
const ARGON2ID: u8 = 2;
const SALT: usize = 16;
const RECORD: usize = 1 + 12 + SALT;

/// OWASP's first Argon2id setting (19 MiB, 2 passes, 1 lane): tens of milliseconds natively,
/// a fraction of a second in a browser.
const DEFAULT: Cost = Cost {
    m_kib: 19 * 1024,
    t: 2,
    p: 1,
};
/// Bounds on a record read from a store, which anyone holding the file can edit: close to the
/// default, so an edited record cannot make opening take much longer or much more memory.
const MAX_M_KIB: u32 = 64 * 1024;
const MAX_T: u32 = 4;
const MAX_P: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Cost {
    pub m_kib: u32,
    pub t: u32,
    pub p: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Kdf {
    /// `HKDF-SHA512(no salt, passphrase)`: stores made before the KDF row existed.
    Legacy,
    Argon2id {
        cost: Cost,
        salt: [u8; SALT],
    },
}

impl Kdf {
    /// A fresh record for a new store: the default cost and a random salt.
    pub(super) fn new() -> Result<Kdf, StoreError> {
        let mut salt = [0u8; SALT];
        getrandom::fill(&mut salt)
            .map_err(|e| StoreError::Io(std::io::Error::other(e.to_string())))?;
        Ok(Kdf::Argon2id {
            cost: DEFAULT,
            salt,
        })
    }

    pub(super) fn encode(&self) -> Option<Vec<u8>> {
        let Kdf::Argon2id { cost, salt } = self else {
            return None;
        };
        let mut out = vec![ARGON2ID];
        for n in [cost.m_kib, cost.t, cost.p] {
            out.extend_from_slice(&n.to_le_bytes());
        }
        out.extend_from_slice(salt);
        Some(out)
    }

    /// The record in a `kdf` row; a cost past the bounds is `Corrupt` before any memory is taken.
    pub(super) fn decode(row: &[u8]) -> Result<Kdf, StoreError> {
        let bad = |why: &str| StoreError::Corrupt(format!("the key derivation record {why}"));
        let rec: &[u8; RECORD] = row.try_into().map_err(|_| bad("has the wrong length"))?;
        if rec[0] != ARGON2ID {
            return Err(bad("names an unknown function"));
        }
        let word = |i: usize| u32::from_le_bytes([rec[i], rec[i + 1], rec[i + 2], rec[i + 3]]);
        let cost = Cost {
            m_kib: word(1),
            t: word(5),
            p: word(9),
        };
        let fits = (1..=MAX_P).contains(&cost.p)
            && (1..=MAX_T).contains(&cost.t)
            && (8 * cost.p..=MAX_M_KIB).contains(&cost.m_kib);
        if !fits {
            return Err(bad("asks for a cost out of bounds"));
        }
        let mut salt = [0u8; SALT];
        salt.copy_from_slice(&rec[1 + 12..]);
        Ok(Kdf::Argon2id { cost, salt })
    }

    /// The sealer for `key` under this record; Argon2id runs here.
    pub(super) fn sealer(&self, key: &str) -> Result<Sealer, StoreError> {
        match self {
            Kdf::Legacy => Ok(Sealer::legacy(key)),
            Kdf::Argon2id { cost, salt } => {
                Ok(Sealer::stretched(&*Self::stretch(*cost, salt, key)?))
            }
        }
    }

    fn stretch(cost: Cost, salt: &[u8], key: &str) -> Result<Zeroizing<[u8; 32]>, StoreError> {
        let params = Params::new(cost.m_kib, cost.t, cost.p, Some(32))
            .map_err(|_| StoreError::Corrupt("the key derivation cost is not valid".into()))?;
        let mut out = Zeroizing::new([0u8; 32]);
        Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
            .hash_password_into(key.as_bytes(), salt, &mut *out)
            .map_err(|_| StoreError::Invalid("the key cannot be stretched"))?;
        Ok(out)
    }
}
