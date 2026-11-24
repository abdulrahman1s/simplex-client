//! The SQLite file holds no plaintext, and deleted rows leave no ciphertext behind.
use super::Dir;
use crate::store::Store;

fn disk_bytes(d: &Dir) -> Vec<u8> {
    let mut all = Vec::new();
    for e in std::fs::read_dir(&d.0).unwrap() {
        all.extend(std::fs::read(e.unwrap().path()).unwrap());
    }
    all
}

fn contains(hay: &[u8], needle: &[u8]) -> bool {
    hay.windows(needle.len()).any(|w| w == needle)
}

#[test]
fn values_on_disk_contain_no_plaintext() {
    let d = Dir::new();
    {
        let mut s = Store::open(&d.db(), "k").unwrap();
        s.put("agent", b"KEYMARKER-key", b"VALMARKER-kv").unwrap();
        s.insert_item(b"CHATMARKER-ref", b"ITEMMARKER-text")
            .unwrap();
        s.put_file(1, b"FILEMARKER-rec").unwrap();
        s.next_id("friends").unwrap();
        // Still in the WAL, then checkpointed on close: check both states.
        let live = disk_bytes(&d);
        assert!(!contains(&live, b"MARKER"));
    }
    let closed = disk_bytes(&d);
    assert!(
        contains(&closed, b"agent"),
        "namespace tags are the documented clear part"
    );
    assert!(!contains(&closed, b"MARKER"));
}

#[test]
fn a_deleted_row_is_overwritten_in_the_file() {
    let d = Dir::new();
    let sealed;
    {
        let mut s = Store::open(&d.db(), "k").unwrap();
        s.put_file(1, &[7u8; 300]).unwrap();
        sealed = s.rows.file_get(1).unwrap().unwrap();
        s.delete_file(1).unwrap();
    }
    assert!(!contains(&disk_bytes(&d), &sealed[..64]));
}
