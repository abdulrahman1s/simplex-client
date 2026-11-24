//! Every SQLite row read out into `Tables`, for a snapshot of a file store, and the counters alone.

use super::db::Db;
use crate::store::StoreError;
use crate::store::counters::Marks;
use crate::store::memory::Tables;

type Blob = Vec<u8>;

pub(super) fn dump(db: &mut Db) -> Result<Tables, StoreError> {
    db.run(async |c| {
        let meta: Vec<(String, Blob)> = ormlite::query_as("SELECT k, v FROM meta")
            .fetch_all(&mut *c)
            .await?;
        let kv: Vec<(Blob, Blob)> = ormlite::query_as("SELECT k, v FROM kv")
            .fetch_all(&mut *c)
            .await?;
        let items: Vec<(Blob, i64, Blob)> = ormlite::query_as("SELECT chat, id, v FROM items")
            .fetch_all(&mut *c)
            .await?;
        let files: Vec<(i64, Blob)> = ormlite::query_as("SELECT id, v FROM files")
            .fetch_all(&mut *c)
            .await?;
        let seq: Vec<(String, i64)> = ormlite::query_as("SELECT name, next FROM seq")
            .fetch_all(&mut *c)
            .await?;
        let last: Option<(i64,)> =
            ormlite::query_as("SELECT seq FROM sqlite_sequence WHERE name = 'items'")
                .fetch_optional(&mut *c)
                .await?;
        Ok(Tables {
            meta: meta.into_iter().collect(),
            kv: kv.into_iter().collect(),
            items: items.into_iter().map(|(ch, id, v)| ((ch, id), v)).collect(),
            files: files.into_iter().collect(),
            seq: seq.into_iter().collect(),
            last_item: last.map_or(0, |(n,)| n),
        })
    })
}

pub(super) fn counters(db: &mut Db) -> Result<Marks, StoreError> {
    db.run(async |c| {
        let seq: Vec<(String, i64)> = ormlite::query_as("SELECT name, next FROM seq")
            .fetch_all(&mut *c)
            .await?;
        let last: Option<(i64,)> =
            ormlite::query_as("SELECT seq FROM sqlite_sequence WHERE name = 'items'")
                .fetch_optional(&mut *c)
                .await?;
        Ok(Marks {
            last_item: last.map_or(0, |(n,)| n),
            seq: seq.into_iter().collect(),
        })
    })
}
