//! Every SQLite row read out into `Tables`, for a snapshot of a file store, and the counters alone.

use rusqlite::OptionalExtension;

use super::db::Db;
use crate::store::StoreError;
use crate::store::counters::Marks;
use crate::store::memory::Tables;

pub(super) fn dump(db: &mut Db) -> Result<Tables, StoreError> {
    let c = &db.conn;
    let meta = c
        .prepare_cached("SELECT k, v FROM meta")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let kv = c
        .prepare_cached("SELECT k, v FROM kv")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let items = c
        .prepare_cached("SELECT chat, id, v FROM items")?
        .query_map([], |r| Ok(((r.get(0)?, r.get(1)?), r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    let files = c
        .prepare_cached("SELECT id, v FROM files")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let marks = counters(db)?;
    Ok(Tables {
        meta,
        kv,
        items,
        files,
        seq: marks.seq,
        last_item: marks.last_item,
    })
}

pub(super) fn counters(db: &mut Db) -> Result<Marks, StoreError> {
    let c = &db.conn;
    let seq = c
        .prepare_cached("SELECT name, next FROM seq")?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let last: Option<i64> = c
        .prepare_cached("SELECT seq FROM sqlite_sequence WHERE name = 'items'")?
        .query_row([], |r| r.get(0))
        .optional()?;
    Ok(Marks {
        last_item: last.unwrap_or(0),
        seq,
    })
}
