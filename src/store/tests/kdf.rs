//! The store key: Argon2id with a per-store salt, old HKDF stores still open, and no empty key.
use super::{Bed, both};
use crate::store::kdf::{KDF_ROW, Kdf};
use crate::store::seal::Sealer;
use crate::store::{Store, StoreError};

const LEGACY_SNAPSHOT: &[u8] = include_bytes!("fixtures/legacy-v1.snapshot");
const LEGACY_EMPTY_KEY: &[u8] = include_bytes!("fixtures/legacy-empty-key.snapshot");
#[cfg(feature = "native")]
const LEGACY_DB: &[u8] = include_bytes!("fixtures/legacy-v1.db");

fn record(s: &mut Store) -> Vec<u8> {
    s.rows.meta_get(KDF_ROW).unwrap().expect("a kdf row")
}

fn a_new_store_is_keyed_by_argon2id_with_its_own_salt(d: Bed) {
    let mut a = d.store("same passphrase");
    let mut b = Store::open_memory("same passphrase").unwrap();
    let (ra, rb) = (record(&mut a), record(&mut b));
    let (Kdf::Argon2id { cost, salt: sa }, Kdf::Argon2id { salt: sb, .. }) =
        (Kdf::decode(&ra).unwrap(), Kdf::decode(&rb).unwrap())
    else {
        panic!("not Argon2id");
    };
    assert_eq!((cost.m_kib, cost.t, cost.p), (19 * 1024, 2, 1));
    assert_ne!(sa, sb, "every store draws its own salt");
    let check = a.rows.meta_get("check").unwrap().unwrap();
    let hkdf_only = Sealer::legacy("same passphrase");
    assert!(
        hkdf_only.open(&[b"meta", b"check"], &check).is_err(),
        "the check row is not under the unstretched key"
    );
}

fn a_changed_salt_is_a_wrong_key(mut d: Bed) {
    let mut s = d.store("k");
    let mut rec = record(&mut s);
    *rec.last_mut().unwrap() ^= 1;
    s.rows.meta_put(KDF_ROW, rec).unwrap();
    d.close(s);
    assert!(matches!(d.open("k"), Err(StoreError::WrongKey)));
}

fn a_new_store_needs_a_non_empty_key(d: Bed) {
    assert!(matches!(d.open(""), Err(StoreError::Invalid(_))));
}

both!(
    a_new_store_is_keyed_by_argon2id_with_its_own_salt,
    a_changed_salt_is_a_wrong_key,
    a_new_store_needs_a_non_empty_key,
);

#[test]
fn a_cost_out_of_bounds_is_corrupt_before_any_memory_is_taken() {
    let mut s = Store::open_memory("k").unwrap();
    let good = record(&mut s);
    for (at, value) in [
        (1, u32::MAX),
        (1, 64 * 1024 + 1),
        (1, 7),
        (5, 0),
        (5, 5),
        (9, 0),
        (9, 5),
    ] {
        let mut rec = good.clone();
        rec[at..at + 4].copy_from_slice(&value.to_le_bytes());
        assert!(
            matches!(Kdf::decode(&rec), Err(StoreError::Corrupt(_))),
            "{at}={value}"
        );
    }
    assert!(Kdf::decode(&good[1..]).is_err());
    assert!(Kdf::decode(&[&[1u8][..], &good[1..]].concat()).is_err());
}

fn reads_the_legacy_rows(s: &mut Store) {
    assert_eq!(
        s.get("agent", b"conn-1").unwrap().as_deref(),
        Some(&b"legacy state"[..])
    );
    let items = s.page(b"direct:1", None, 10).unwrap();
    assert_eq!(items.len(), 2);
    assert_eq!(items[1].1, b"legacy second");
    assert_eq!(s.get_file(7).unwrap().as_deref(), Some(&b"legacy file"[..]));
    assert_eq!(s.next_id("friends").unwrap(), 3);
    assert!(s.insert_item(b"direct:1", b"new").unwrap() > items[1].0);
}

#[test]
fn a_legacy_hkdf_snapshot_still_opens_and_reads_back() {
    assert!(matches!(
        Store::open_snapshot(LEGACY_SNAPSHOT, "wrong"),
        Err(StoreError::WrongKey)
    ));
    let mut s = Store::open_snapshot(LEGACY_SNAPSHOT, "legacy key").unwrap();
    assert_eq!(s.rows.meta_get(KDF_ROW).unwrap(), None, "not upgraded");
    let again = s.export_snapshot().unwrap();
    reads_the_legacy_rows(&mut s);
    reads_the_legacy_rows(&mut Store::open_snapshot(&again, "legacy key").unwrap());
}

#[test]
fn a_legacy_store_sealed_with_the_empty_key_still_opens() {
    let mut s = Store::open_snapshot(LEGACY_EMPTY_KEY, "").unwrap();
    assert_eq!(s.get("agent", b"k").unwrap().as_deref(), Some(&b"v"[..]));
}

#[cfg(feature = "native")]
#[test]
fn a_legacy_hkdf_sqlite_store_still_opens_and_reads_back() {
    let d = super::Dir::new();
    std::fs::write(d.db(), LEGACY_DB).unwrap();
    assert!(matches!(
        Store::open(&d.db(), "wrong"),
        Err(StoreError::WrongKey)
    ));
    let mut s = Store::open(&d.db(), "legacy key").unwrap();
    reads_the_legacy_rows(&mut s);
    drop(s);
    let mut s = Store::open(&d.db(), "legacy key").unwrap();
    assert_eq!(s.rows.meta_get(KDF_ROW).unwrap(), None, "not upgraded");
}
