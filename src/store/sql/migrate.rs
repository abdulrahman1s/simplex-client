//! Numbered SQL migrations, each in its own transaction ending with `user_version`.

use rusqlite::Connection;

use crate::store::StoreError;

const MIGRATIONS: &[&str] = &[include_str!("../migrations/0001_init.sql")];

pub(super) fn run(c: &Connection) -> Result<(), StoreError> {
    let version: i64 = c.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let done = usize::try_from(version).unwrap_or(usize::MAX);
    if done > MIGRATIONS.len() {
        return Err(StoreError::Corrupt(
            "the store was written by a newer version".into(),
        ));
    }
    for (sql, version) in MIGRATIONS.iter().zip(1_i64..).skip(done) {
        c.execute_batch("BEGIN IMMEDIATE")?;
        let r = c
            .execute_batch(sql)
            .and_then(|()| c.pragma_update(None, "user_version", version));
        match r {
            Ok(()) => c.execute_batch("COMMIT")?,
            Err(e) => {
                let _ = c.execute_batch("ROLLBACK"); // the migration's error is the one to report
                return Err(e.into());
            }
        }
    }
    Ok(())
}
