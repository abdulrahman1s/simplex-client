//! Snapshots: only ciphertext, the key check, total parsing, and either backend saved.
use super::{Bed, both};
use crate::store::{Store, StoreError};

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

fn filled(key: &str) -> Store {
    let mut s = Store::open_memory(key).unwrap();
    s.put("agent", b"KEYMARKER-key", b"VALMARKER-kv").unwrap();
    s.insert_item(b"CHATMARKER-ref", b"ITEMMARKER-text")
        .unwrap();
    s.put_file(1, b"FILEMARKER-rec").unwrap();
    s.next_id("friends").unwrap();
    s
}

#[test]
fn a_snapshot_holds_no_plaintext_only_namespaces_and_ciphertext() {
    let bytes = filled("k").export_snapshot().unwrap();
    assert!(!contains(&bytes, b"MARKER"));
    assert!(contains(&bytes, b"agent"), "namespaces are the clear part");
}

#[test]
fn a_snapshot_opens_only_with_its_key() {
    let bytes = filled("right").export_snapshot().unwrap();
    assert!(matches!(
        Store::open_snapshot(&bytes, "wrong"),
        Err(StoreError::WrongKey)
    ));
    let mut s = Store::open_snapshot(&bytes, "right").unwrap();
    assert_eq!(
        s.get("agent", b"KEYMARKER-key").unwrap().as_deref(),
        Some(&b"VALMARKER-kv"[..])
    );
}

#[test]
fn a_snapshot_round_trips_byte_for_byte() {
    let bytes = filled("k").export_snapshot().unwrap();
    let again = Store::open_snapshot(&bytes, "k")
        .unwrap()
        .export_snapshot()
        .unwrap();
    assert_eq!(bytes, again);
}

#[test]
fn every_cut_or_damaged_snapshot_is_corrupt_and_never_panics() {
    let bytes = filled("k").export_snapshot().unwrap();
    for n in 0..bytes.len() {
        assert!(
            matches!(
                Store::open_snapshot(&bytes[..n], "k"),
                Err(StoreError::Corrupt(_))
            ),
            "cut at {n}"
        );
    }
    let mut longer = bytes.clone();
    longer.push(0);
    assert!(matches!(
        Store::open_snapshot(&longer, "k"),
        Err(StoreError::Corrupt(_))
    ));
    let mut huge_count = bytes[..12].to_vec();
    huge_count.extend_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        Store::open_snapshot(&huge_count, "k"),
        Err(StoreError::Corrupt(_))
    ));
}

#[test]
fn a_snapshot_of_another_version_is_refused() {
    let mut bytes = filled("k").export_snapshot().unwrap();
    bytes[8] = 3;
    assert!(matches!(
        Store::open_snapshot(&bytes, "k"),
        Err(StoreError::Corrupt(_))
    ));
}

/// The sealed length of file row 1 after storing `v`.
fn sealed_len(s: &mut Store, v: &[u8]) -> usize {
    s.put_file(1, v).unwrap();
    s.rows.file_get(1).unwrap().unwrap().len()
}

fn padding_hides_small_size_differences(d: Bed) {
    let mut s = d.store("k");
    let sizes: Vec<usize> = [0usize, 1, 20, 40, 59]
        .iter()
        .map(|n| sealed_len(&mut s, &vec![b'a'; *n]))
        .collect();
    assert!(sizes.windows(2).all(|w| w[0] == w[1]), "{sizes:?}");
    assert!(sealed_len(&mut s, &[b'a'; 61]) > sizes[0]);
}

fn either_backend_exports_a_snapshot_the_memory_store_opens(d: Bed) {
    let mut s = d.store("k");
    s.put("agent", b"a", b"1").unwrap();
    let id = s.insert_item(b"c", b"hi").unwrap();
    s.next_id("groups").unwrap();
    let bytes = s.export_snapshot().unwrap();
    let mut m = Store::open_snapshot(&bytes, "k").unwrap();
    assert_eq!(m.get("agent", b"a").unwrap().as_deref(), Some(&b"1"[..]));
    assert_eq!(m.page(b"c", None, 5).unwrap(), vec![(id, b"hi".to_vec())]);
    assert_eq!(m.next_id("groups").unwrap(), 2);
    assert!(m.insert_item(b"c", b"x").unwrap() > id);
}

both!(
    padding_hides_small_size_differences,
    either_backend_exports_a_snapshot_the_memory_store_opens,
);
