//! A SQLite file changed without the key: rewound or missing counters are caught.
use super::Dir;
use crate::store::{Store, StoreError};

/// Runs `sql` on the file through a connection of its own.
fn tamper(d: &Dir, sql: &str) {
    let c = rusqlite::Connection::open(d.db()).unwrap();
    c.execute_batch(sql).unwrap();
}

fn reopen(d: &Dir) -> Result<Store, StoreError> {
    Store::open(&d.db(), "k")
}

fn used(d: &Dir) {
    let mut s = reopen(d).unwrap();
    s.next_id("friends").unwrap();
    s.next_id("friends").unwrap();
    let id = s.insert_item(b"c", b"x").unwrap();
    s.delete_item(b"c", id).unwrap();
}

#[test]
fn a_rewound_id_counter_in_the_file_is_corrupt_on_open() {
    let d = Dir::new();
    used(&d);
    tamper(&d, "UPDATE seq SET next = 0");
    assert!(matches!(reopen(&d), Err(StoreError::Corrupt(_))));
}

#[test]
fn a_rewound_item_counter_in_the_file_is_corrupt_on_open() {
    let d = Dir::new();
    used(&d);
    tamper(
        &d,
        "UPDATE sqlite_sequence SET seq = 0 WHERE name = 'items'",
    );
    assert!(matches!(reopen(&d), Err(StoreError::Corrupt(_))));
}

#[test]
fn a_file_without_its_sealed_counters_is_corrupt() {
    let d = Dir::new();
    used(&d);
    tamper(&d, "DELETE FROM meta WHERE k = 'counters'");
    assert!(matches!(reopen(&d), Err(StoreError::Corrupt(_))));
}

#[test]
fn a_counter_rewound_while_the_store_is_open_fails_the_next_id() {
    let d = Dir::new();
    let mut s = reopen(&d).unwrap();
    s.next_id("friends").unwrap();
    tamper(&d, "UPDATE seq SET next = 0");
    assert!(matches!(s.next_id("friends"), Err(StoreError::Corrupt(_))));
}

#[test]
fn a_legacy_file_gets_counter_marks_on_its_first_open() {
    let d = Dir::new();
    std::fs::write(d.db(), include_bytes!("fixtures/legacy-v1.db")).unwrap();
    drop(Store::open(&d.db(), "legacy key").unwrap());
    tamper(&d, "UPDATE seq SET next = 0");
    assert!(matches!(
        Store::open(&d.db(), "legacy key"),
        Err(StoreError::Corrupt(_))
    ));
}
