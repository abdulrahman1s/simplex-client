//! The id counters' high-water marks, sealed in one meta row, so a rewound clear counter is caught.

// The clear counters (`seq`, the last item id) are what the backends increment; the sealed row
// only moves forward with them. A clear counter below its mark was rewound. Replaying an older
// sealed row together with older counters is a row rollback, which only a snapshot's MAC sees.

use std::collections::BTreeMap;

use super::StoreError;
use super::rows::Rows;
use super::seal::Sealer;

const ROW: &str = "counters";
const AAD: [&[u8]; 2] = [b"meta", ROW.as_bytes()];

/// The marks: the last item id and each named counter's last value.
#[derive(Debug, Default, PartialEq, Eq)]
pub(super) struct Marks {
    pub last_item: i64,
    pub seq: BTreeMap<String, i64>,
}

fn rewound() -> StoreError {
    StoreError::Corrupt("an id counter was rewound".into())
}

fn bad() -> StoreError {
    StoreError::Corrupt("bad counters record".into())
}

impl Marks {
    fn encode(&self) -> Vec<u8> {
        let mut out = self.last_item.to_le_bytes().to_vec();
        for (name, next) in &self.seq {
            out.extend_from_slice(&(name.len() as u32).to_le_bytes());
            out.extend_from_slice(name.as_bytes());
            out.extend_from_slice(&next.to_le_bytes());
        }
        out
    }

    fn decode(mut b: &[u8]) -> Result<Marks, StoreError> {
        let mut take = |n: usize| -> Result<&[u8], StoreError> {
            let (head, rest) = b.split_at_checked(n).ok_or_else(bad)?;
            b = rest;
            Ok(head)
        };
        let i64_at = |s: &[u8]| s.try_into().map(i64::from_le_bytes).map_err(|_| bad());
        let mut marks = Marks {
            last_item: i64_at(take(8)?)?,
            seq: BTreeMap::new(),
        };
        while !b.is_empty() {
            let len: [u8; 4] = b.get(..4).and_then(|l| l.try_into().ok()).ok_or_else(bad)?;
            let (name, rest) = b[4..]
                .split_at_checked(u32::from_le_bytes(len) as usize)
                .ok_or_else(bad)?;
            let (next, rest) = rest.split_first_chunk::<8>().ok_or_else(bad)?;
            let name = String::from_utf8(name.to_vec()).map_err(|_| bad())?;
            marks.seq.insert(name, i64::from_le_bytes(*next));
            b = rest;
        }
        Ok(marks)
    }
}

fn load(rows: &mut dyn Rows, s: &Sealer) -> Result<Option<Marks>, StoreError> {
    let Some(sealed) = rows.meta_get(ROW)? else {
        return Ok(None);
    };
    Marks::decode(&s.open(&AAD, &sealed)?).map(Some)
}

fn save(rows: &mut dyn Rows, s: &Sealer, m: &Marks) -> Result<(), StoreError> {
    rows.meta_put(ROW, s.seal(&AAD, &m.encode())?)
}

/// A new store's marks: nothing given out yet.
pub(super) fn create(rows: &mut dyn Rows, s: &Sealer) -> Result<(), StoreError> {
    save(rows, s, &Marks::default())
}

/// On open: every clear counter at or above its mark. A store from before the marks (`required`
/// false) gets them from its counters as they are now.
pub(super) fn check(rows: &mut dyn Rows, s: &Sealer, required: bool) -> Result<(), StoreError> {
    let clear = rows.counters()?;
    let Some(marks) = load(rows, s)? else {
        if required {
            return Err(StoreError::Corrupt(
                "the sealed counters are missing".into(),
            ));
        }
        return save(rows, s, &clear);
    };
    // A rolled-back insert can leave the clear item counter above its mark, never below.
    let below = |name: &String, mark: &i64| clear.seq.get(name).copied().unwrap_or(0) < *mark;
    if clear.last_item < marks.last_item || marks.seq.iter().any(|(n, m)| below(n, m)) {
        return Err(rewound());
    }
    Ok(())
}

/// Moves `name`'s mark to the value the backend just gave out.
pub(super) fn bump_seq(
    rows: &mut dyn Rows,
    s: &Sealer,
    name: &str,
    next: i64,
) -> Result<(), StoreError> {
    let mut m = load(rows, s)?.unwrap_or_default();
    if next <= m.seq.get(name).copied().unwrap_or(0) {
        return Err(rewound());
    }
    m.seq.insert(name.to_owned(), next);
    save(rows, s, &m)
}

/// Moves the item mark to the id the backend just gave out.
pub(super) fn bump_item(rows: &mut dyn Rows, s: &Sealer, id: i64) -> Result<(), StoreError> {
    let mut m = load(rows, s)?.unwrap_or_default();
    if id <= m.last_item {
        return Err(rewound());
    }
    m.last_item = id;
    save(rows, s, &m)
}
