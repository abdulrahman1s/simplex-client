//! A snapshot changed without the key is refused as a whole.
use crate::store::snapshot::{decode, encode};
use crate::store::{Store, StoreError};

const LEGACY_SNAPSHOT: &[u8] = include_bytes!("fixtures/legacy-v1.snapshot");

/// The snapshot's tables after `edit`, re-encoded with the tag it had: all a keyless editor can do.
fn edited(bytes: &[u8], edit: impl FnOnce(&mut crate::store::memory::Tables)) -> Vec<u8> {
    let d = decode(bytes).unwrap();
    let mut t = d.tables;
    edit(&mut t);
    [encode(&t), d.tag.unwrap().1.to_vec()].concat()
}

fn corrupt(bytes: &[u8]) -> bool {
    matches!(
        Store::open_snapshot(bytes, "k"),
        Err(StoreError::Corrupt(_))
    )
}

#[test]
fn a_snapshot_with_one_row_rolled_back_or_deleted_is_refused() {
    let mut s = Store::open_memory("k").unwrap();
    s.put("ratchet", b"conn1", b"state v1").unwrap();
    s.put("ratchet", b"conn2", b"other v1").unwrap();
    let old = decode(&s.export_snapshot().unwrap()).unwrap().tables;
    s.put("ratchet", b"conn1", b"state v2").unwrap();
    s.put("ratchet", b"conn2", b"other v2").unwrap();
    let now = s.export_snapshot().unwrap();
    assert!(Store::open_snapshot(&now, "k").is_ok());
    let row = old.kv.keys().next().unwrap().clone();
    assert!(corrupt(&edited(&now, |t| {
        t.kv.insert(row.clone(), old.kv[&row].clone());
    })));
    assert!(corrupt(&edited(&now, |t| drop(t.kv.remove(&row)))));
}

#[test]
fn a_snapshot_with_a_rewound_counter_is_refused() {
    let mut s = Store::open_memory("k").unwrap();
    s.next_id("agent.conn").unwrap();
    s.next_id("agent.conn").unwrap();
    s.insert_item(b"c", b"x").unwrap();
    let bytes = s.export_snapshot().unwrap();
    assert!(corrupt(&edited(&bytes, |t| {
        t.seq.insert("agent.conn".into(), 0);
    })));
    assert!(corrupt(&edited(&bytes, |t| t.last_item = 0)));
}

/// `bytes` as the first format: version 1 and no tag.
fn untagged(bytes: &[u8]) -> Vec<u8> {
    let mut v = bytes[..bytes.len() - 32].to_vec();
    v[8] = 1;
    v
}

#[test]
fn a_snapshot_cut_down_to_the_untagged_format_is_refused() {
    let bytes = Store::open_memory("k").unwrap().export_snapshot().unwrap();
    assert!(corrupt(&untagged(&bytes)));
    let upgraded = Store::open_snapshot(LEGACY_SNAPSHOT, "legacy key")
        .unwrap()
        .export_snapshot()
        .unwrap();
    assert!(matches!(
        Store::open_snapshot(&untagged(&upgraded), "legacy key"),
        Err(StoreError::Corrupt(_))
    ));
}
