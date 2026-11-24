//! The row operations as SQL statements with bound arguments.

use super::db::{Db, exec};
use crate::store::StoreError;
use crate::store::counters::Marks;
use crate::store::memory::Tables;
use crate::store::rows::{KvRow, Rows};

type Blob = Vec<u8>;

impl Rows for Db {
    fn begin(&mut self) -> Result<(), StoreError> {
        self.run(async |c| exec(c, "BEGIN IMMEDIATE").await)
    }

    fn commit(&mut self) -> Result<(), StoreError> {
        self.run(async |c| exec(c, "COMMIT").await)
    }

    fn rollback(&mut self) {
        let _ = self.run(async |c| exec(c, "ROLLBACK").await); // the failing op's error is the one to report
    }

    fn meta_get(&mut self, k: &str) -> Result<Option<Blob>, StoreError> {
        let q = ormlite::query_as("SELECT v FROM meta WHERE k = ?1").bind(k);
        let row: Option<(Blob,)> = self.run(async |c| Ok(q.fetch_optional(c).await?))?;
        Ok(row.map(|(v,)| v))
    }

    fn meta_put(&mut self, k: &str, v: Blob) -> Result<(), StoreError> {
        let q = ormlite::query(
            "INSERT INTO meta (k, v) VALUES (?1, ?2) ON CONFLICT (k) DO UPDATE SET v = excluded.v",
        );
        let q = q.bind(k).bind(v);
        self.run(async |c| Ok(q.execute(c).await.map(drop)?))
    }

    fn kv_get(&mut self, k: &[u8]) -> Result<Option<Blob>, StoreError> {
        let q = ormlite::query_as("SELECT v FROM kv WHERE k = ?1").bind(k);
        let row: Option<(Blob,)> = self.run(async |c| Ok(q.fetch_optional(c).await?))?;
        Ok(row.map(|(v,)| v))
    }

    fn kv_put(&mut self, k: Blob, v: Blob) -> Result<(), StoreError> {
        let q = ormlite::query(
            "INSERT INTO kv (k, v) VALUES (?1, ?2) ON CONFLICT (k) DO UPDATE SET v = excluded.v",
        );
        let q = q.bind(k).bind(v);
        self.run(async |c| Ok(q.execute(c).await.map(drop)?))
    }

    fn kv_delete(&mut self, k: &[u8]) -> Result<(), StoreError> {
        let q = ormlite::query("DELETE FROM kv WHERE k = ?1").bind(k);
        self.run(async |c| Ok(q.execute(c).await.map(drop)?))
    }

    fn kv_range(&mut self, lo: &[u8], hi: &[u8]) -> Result<Vec<KvRow>, StoreError> {
        let q = ormlite::query_as("SELECT k, v FROM kv WHERE k >= ?1 AND k < ?2");
        let q = q.bind(lo).bind(hi);
        self.run(async |c| Ok(q.fetch_all(c).await?))
    }

    fn item_new(&mut self, chat: &[u8]) -> Result<i64, StoreError> {
        let q = ormlite::query("INSERT INTO items (chat, v) VALUES (?1, x'')").bind(chat);
        self.run(async |c| Ok(q.execute(c).await?.last_insert_rowid()))
    }

    fn item_set(&mut self, chat: &[u8], id: i64, v: Blob) -> Result<bool, StoreError> {
        let q = ormlite::query("UPDATE items SET v = ?1 WHERE id = ?2 AND chat = ?3");
        let q = q.bind(v).bind(id).bind(chat);
        self.run(async |c| Ok(q.execute(c).await?.rows_affected() > 0))
    }

    fn item_delete(&mut self, chat: &[u8], id: i64) -> Result<(), StoreError> {
        let q = ormlite::query("DELETE FROM items WHERE id = ?1 AND chat = ?2");
        let q = q.bind(id).bind(chat);
        self.run(async |c| Ok(q.execute(c).await.map(drop)?))
    }

    fn item_page(
        &mut self,
        chat: &[u8],
        before: i64,
        count: u32,
    ) -> Result<Vec<(i64, Blob)>, StoreError> {
        let q = ormlite::query_as(
            "SELECT id, v FROM items WHERE chat = ?1 AND id < ?2 ORDER BY id DESC LIMIT ?3",
        );
        let q = q.bind(chat).bind(before).bind(i64::from(count));
        self.run(async |c| Ok(q.fetch_all(c).await?))
    }

    fn file_get(&mut self, id: i64) -> Result<Option<Blob>, StoreError> {
        let q = ormlite::query_as("SELECT v FROM files WHERE id = ?1").bind(id);
        let row: Option<(Blob,)> = self.run(async |c| Ok(q.fetch_optional(c).await?))?;
        Ok(row.map(|(v,)| v))
    }

    fn file_put(&mut self, id: i64, v: Blob) -> Result<(), StoreError> {
        let q = ormlite::query(
            "INSERT INTO files (id, v) VALUES (?1, ?2) ON CONFLICT (id) DO UPDATE SET v = excluded.v",
        );
        let q = q.bind(id).bind(v);
        self.run(async |c| Ok(q.execute(c).await.map(drop)?))
    }

    fn file_delete(&mut self, id: i64) -> Result<(), StoreError> {
        let q = ormlite::query("DELETE FROM files WHERE id = ?1").bind(id);
        self.run(async |c| Ok(q.execute(c).await.map(drop)?))
    }

    fn seq_next(&mut self, name: &str) -> Result<i64, StoreError> {
        let q = ormlite::query_as(
            "INSERT INTO seq (name, next) VALUES (?1, 1) \
             ON CONFLICT (name) DO UPDATE SET next = next + 1 RETURNING next",
        );
        let q = q.bind(name);
        let (next,): (i64,) = self.run(async |c| Ok(q.fetch_one(c).await?))?;
        Ok(next)
    }

    fn counters(&mut self) -> Result<Marks, StoreError> {
        super::dump::counters(self)
    }

    fn dump(&mut self) -> Result<Tables, StoreError> {
        super::dump::dump(self)
    }
}
