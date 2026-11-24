//! Deltas: a snapshot and the changes after it open to the store as it was, and a bad chain is refused.
use crate::store::{Op, Store, StoreError};

const LEGACY_SNAPSHOT: &[u8] = include_bytes!("fixtures/legacy-v1.snapshot");

/// A small deterministic generator, so a failing run can be repeated.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1);
        (self.0 >> 33) % n
    }
}

/// One random write of any kind the store has: kv, items, files, counters, and a batch that fails.
fn write(s: &mut Store, r: &mut Lcg) {
    let key = format!("k{}", r.next(40)).into_bytes();
    let chat = format!("c{}", r.next(4)).into_bytes();
    match r.next(8) {
        0 | 1 => s.put("ns", &key, &r.next(1000).to_le_bytes()).unwrap(),
        2 => s.delete("ns", &key).unwrap(),
        3 => drop(s.insert_item(&chat, b"text").unwrap()),
        4 => {
            let page = s.page(&chat, None, 5).unwrap();
            if let Some((id, _)) = page.first() {
                s.delete_item(&chat, *id).unwrap();
            }
        }
        5 => s
            .put_file(i64::try_from(r.next(5)).unwrap(), b"file")
            .unwrap(),
        6 => drop(s.next_id("friends").unwrap()),
        _ => {
            let id = i64::try_from(r.next(5)).unwrap();
            let ops = vec![
                Op::Delete { ns: "ns", key },
                Op::DeleteFile { id },
                Op::InsertItem {
                    chat_ref: chat,
                    value: b"undone".to_vec(),
                },
            ];
            assert!(s.apply(failing(ops)).is_err());
        }
    }
}

fn rows(s: &mut Store) -> crate::store::memory::Tables {
    s.rows.dump().unwrap()
}

#[test]
fn a_snapshot_and_the_changes_after_it_open_to_the_same_rows() {
    for seed in 1..=20 {
        round_trip(seed);
    }
}

fn round_trip(seed: u64) {
    let mut r = Lcg(seed);
    let mut s = Store::open_memory("k").unwrap();
    let base = s.export_snapshot().unwrap();
    let mut changes = Vec::new();
    for _ in 0..30 {
        for _ in 0..r.next(12) {
            write(&mut s, &mut r);
        }
        changes.push(s.export_changes().unwrap());
    }
    let mut again = Store::open_snapshot_with_changes(&base, &changes, "k").unwrap();
    assert_eq!(rows(&mut again), rows(&mut s), "seed {seed}");
    assert_eq!(again.revision(), 0, "opening is not a change");
    write(&mut again, &mut r);
    write(&mut s, &mut r);
    let next = again.export_changes().unwrap();
    changes.push(next);
    let mut third = Store::open_snapshot_with_changes(&base, &changes, "k").unwrap();
    assert_eq!(
        rows(&mut third),
        rows(&mut again),
        "the chain goes on after opening, seed {seed}"
    );
}

#[test]
fn changes_hold_only_the_rows_written_since_the_last_save() {
    let mut s = Store::open_memory("k").unwrap();
    for i in 0..500 {
        s.put("ns", format!("k{i}").as_bytes(), &[7; 200]).unwrap();
    }
    let whole = s.export_snapshot().unwrap();
    s.put("ns", b"k1", b"new").unwrap();
    let one = s.export_changes().unwrap();
    assert!(
        one.len() * 50 < whole.len(),
        "{} vs {}",
        one.len(),
        whole.len()
    );
}

#[test]
fn a_row_made_and_removed_between_saves_leaves_no_trace() {
    let mut s = Store::open_memory("k").unwrap();
    s.export_snapshot().unwrap();
    let empty = s.export_changes().unwrap();
    for i in 0..100 {
        s.put("ns", format!("t{i}").as_bytes(), b"temp").unwrap();
        s.delete("ns", format!("t{i}").as_bytes()).unwrap();
    }
    let m = s.rows.memory().unwrap();
    assert!(m.changed.kv.0.is_empty(), "the marks do not grow");
    assert_eq!(s.export_changes().unwrap().len(), empty.len());
}

/// A batch that fails on its last op, so everything before it is rolled back.
fn failing(mut ops: Vec<Op>) -> Vec<Op> {
    ops.push(Op::Put {
        ns: "bad\0ns",
        key: b"k".to_vec(),
        value: b"v".to_vec(),
    });
    ops
}

#[test]
fn a_rolled_back_delete_keeps_the_row_in_the_next_changes() {
    let mut s = Store::open_memory("k").unwrap();
    let base = s.export_snapshot().unwrap();
    s.put("ns", b"kept", b"v").unwrap();
    let item = s.insert_item(b"c", b"x").unwrap();
    s.put_file(1, b"f").unwrap();
    let undone = failing(vec![
        Op::Delete {
            ns: "ns",
            key: b"kept".to_vec(),
        },
        Op::DeleteItem {
            chat_ref: b"c".to_vec(),
            id: item,
        },
        Op::DeleteFile { id: 1 },
    ]);
    assert!(s.apply(undone).is_err());
    let change = s.export_changes().unwrap();
    let mut again = Store::open_snapshot_with_changes(&base, &[change], "k").unwrap();
    assert_eq!(rows(&mut again), rows(&mut s));
}

#[test]
fn a_rolled_back_insert_leaves_no_mark() {
    let mut s = Store::open_memory("k").unwrap();
    s.export_snapshot().unwrap();
    let undone = failing(vec![Op::Put {
        ns: "ns",
        key: b"never".to_vec(),
        value: b"v".to_vec(),
    }]);
    assert!(s.apply(undone).is_err());
    assert!(s.rows.memory().unwrap().changed.kv.0.is_empty());
}

#[test]
fn a_row_deleted_after_the_save_is_gone_after_opening() {
    let mut s = Store::open_memory("k").unwrap();
    s.put("ns", b"gone", b"v").unwrap();
    let base = s.export_snapshot().unwrap();
    s.delete("ns", b"gone").unwrap();
    let change = s.export_changes().unwrap();
    let mut again = Store::open_snapshot_with_changes(&base, &[change], "k").unwrap();
    assert_eq!(again.get("ns", b"gone").unwrap(), None);
}

#[test]
fn ids_keep_counting_after_changes_are_opened() {
    let mut s = Store::open_memory("k").unwrap();
    let base = s.export_snapshot().unwrap();
    s.next_id("friends").unwrap();
    let item = s.insert_item(b"c", b"x").unwrap();
    let change = s.export_changes().unwrap();
    let mut again = Store::open_snapshot_with_changes(&base, &[change], "k").unwrap();
    assert_eq!(again.next_id("friends").unwrap(), 2);
    assert!(again.insert_item(b"c", b"y").unwrap() > item);
}

/// A base and three deltas, each changing one row.
fn chain() -> (Vec<u8>, Vec<Vec<u8>>) {
    let mut s = Store::open_memory("k").unwrap();
    let base = s.export_snapshot().unwrap();
    let changes = (0..3)
        .map(|i: u8| {
            s.put("ns", &[i], &[i]).unwrap();
            s.export_changes().unwrap()
        })
        .collect();
    (base, changes)
}

fn open(base: &[u8], changes: &[Vec<u8>]) -> Result<Store, StoreError> {
    Store::open_snapshot_with_changes(base, changes, "k")
}

#[test]
fn a_changed_byte_in_saved_changes_is_corrupt() {
    let (base, mut changes) = chain();
    let last = changes[1].len() - 40;
    changes[1][last] ^= 1;
    assert!(matches!(open(&base, &changes), Err(StoreError::Corrupt(_))));
}

#[test]
fn missing_repeated_or_reordered_changes_do_not_follow() {
    let (base, c) = chain();
    let dropped = [c[0].clone(), c[2].clone()];
    let repeated = [c[0].clone(), c[0].clone()];
    let reordered = [c[1].clone(), c[0].clone()];
    for bad in [&dropped[..], &repeated[..], &reordered[..]] {
        assert!(matches!(
            open(&base, bad),
            Err(StoreError::ChangesDoNotFollow)
        ));
    }
}

#[test]
fn changes_open_only_with_their_key() {
    let (base, c) = chain();
    assert!(matches!(
        Store::open_snapshot_with_changes(&base, &c, "wrong"),
        Err(StoreError::WrongKey)
    ));
}

#[test]
fn changes_of_an_older_snapshot_are_skipped_so_compaction_can_crash_safely() {
    let mut s = Store::open_memory("k").unwrap();
    s.export_snapshot().unwrap();
    s.put("ns", b"a", b"1").unwrap();
    let old = s.export_changes().unwrap();
    // A new whole save replaced the base, and the crash came before the old log was cleared.
    let newer = s.export_snapshot().unwrap();
    s.put("ns", b"b", b"2").unwrap();
    let fresh = s.export_changes().unwrap();
    let mut again = open(&newer, &[old, fresh]).unwrap();
    assert_eq!(rows(&mut again), rows(&mut s));
}

#[test]
fn changes_need_a_whole_save_first() {
    let mut fresh = Store::open_memory("k").unwrap();
    assert!(matches!(
        fresh.export_changes(),
        Err(StoreError::Invalid(_))
    ));
    let mut legacy = Store::open_snapshot(LEGACY_SNAPSHOT, "legacy key").unwrap();
    assert!(matches!(
        legacy.export_changes(),
        Err(StoreError::Invalid(_))
    ));
    let base = legacy.export_snapshot().unwrap();
    legacy.put("ns", b"after", b"v").unwrap();
    let change = legacy.export_changes().unwrap();
    let mut again = Store::open_snapshot_with_changes(&base, &[change], "legacy key").unwrap();
    assert_eq!(
        again.get("ns", b"after").unwrap().as_deref(),
        Some(&b"v"[..])
    );
}
