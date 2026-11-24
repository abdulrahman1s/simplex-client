//! The SQLite connection keeps the settings the store has always run with.
use super::Dir;
use crate::store::sql::Db;

#[test]
fn a_store_file_runs_in_wal_with_secure_delete_foreign_keys_and_a_busy_timeout() {
    let d = Dir::new();
    let db = Db::open(&d.db()).unwrap();
    let text = |p: &str| -> String { db.conn.pragma_query_value(None, p, |r| r.get(0)).unwrap() };
    let int = |p: &str| -> i64 { db.conn.pragma_query_value(None, p, |r| r.get(0)).unwrap() };
    assert_eq!(text("journal_mode"), "wal");
    assert_eq!(int("synchronous"), 1, "NORMAL");
    assert_eq!(int("secure_delete"), 1);
    assert_eq!(int("foreign_keys"), 1);
    assert_eq!(int("busy_timeout"), 5000);
    assert_eq!(int("user_version"), 1, "every migration ran");
}
