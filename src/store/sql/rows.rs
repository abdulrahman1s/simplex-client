//! The row operations as SQL statements with bound arguments.

use rusqlite::{OptionalExtension, params};

use super::db::Db;
use crate::store::StoreError;
use crate::store::counters::Marks;
use crate::store::memory::Tables;
use crate::store::rows::{KvRow, Rows};

type Blob = Vec<u8>;

impl Db {
    fn one(&self, sql: &str, key: impl rusqlite::Params) -> Result<Option<Blob>, StoreError> {
        let mut st = self.conn.prepare_cached(sql)?;
        Ok(st.query_row(key, |r| r.get(0)).optional()?)
    }

    fn write(&self, sql: &str, args: impl rusqlite::Params) -> Result<usize, StoreError> {
        Ok(self.conn.prepare_cached(sql)?.execute(args)?)
    }
}

impl Rows for Db {
    fn begin(&mut self) -> Result<(), StoreError> {
        self.exec("BEGIN IMMEDIATE")
    }

    fn commit(&mut self) -> Result<(), StoreError> {
        self.exec("COMMIT")
    }

    fn rollback(&mut self) {
        let _ = self.exec("ROLLBACK"); // the failing op's error is the one to report
    }

    fn meta_get(&mut self, k: &str) -> Result<Option<Blob>, StoreError> {
        self.one("SELECT v FROM meta WHERE k = ?1", [k])
    }

    fn meta_put(&mut self, k: &str, v: Blob) -> Result<(), StoreError> {
        self.write(
            "INSERT INTO meta (k, v) VALUES (?1, ?2) ON CONFLICT (k) DO UPDATE SET v = excluded.v",
            params![k, v],
        )
        .map(drop)
    }

    fn kv_get(&mut self, k: &[u8]) -> Result<Option<Blob>, StoreError> {
        self.one("SELECT v FROM kv WHERE k = ?1", [k])
    }

    fn kv_put(&mut self, k: Blob, v: Blob) -> Result<(), StoreError> {
        self.write(
            "INSERT INTO kv (k, v) VALUES (?1, ?2) ON CONFLICT (k) DO UPDATE SET v = excluded.v",
            params![k, v],
        )
        .map(drop)
    }

    fn kv_delete(&mut self, k: &[u8]) -> Result<(), StoreError> {
        self.write("DELETE FROM kv WHERE k = ?1", [k]).map(drop)
    }

    fn kv_range(&mut self, lo: &[u8], hi: &[u8]) -> Result<Vec<KvRow>, StoreError> {
        let mut st = self
            .conn
            .prepare_cached("SELECT k, v FROM kv WHERE k >= ?1 AND k < ?2")?;
        let rows = st.query_map([lo, hi], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    fn item_new(&mut self, chat: &[u8]) -> Result<i64, StoreError> {
        let mut st = self
            .conn
            .prepare_cached("INSERT INTO items (chat, v) VALUES (?1, x'')")?;
        Ok(st.insert([chat])?)
    }

    fn item_set(&mut self, chat: &[u8], id: i64, v: Blob) -> Result<bool, StoreError> {
        let changed = self.write(
            "UPDATE items SET v = ?1 WHERE id = ?2 AND chat = ?3",
            params![v, id, chat],
        )?;
        Ok(changed > 0)
    }

    fn item_delete(&mut self, chat: &[u8], id: i64) -> Result<(), StoreError> {
        self.write(
            "DELETE FROM items WHERE id = ?1 AND chat = ?2",
            params![id, chat],
        )
        .map(drop)
    }

    fn item_page(
        &mut self,
        chat: &[u8],
        before: i64,
        count: u32,
    ) -> Result<Vec<(i64, Blob)>, StoreError> {
        let mut st = self.conn.prepare_cached(
            "SELECT id, v FROM items WHERE chat = ?1 AND id < ?2 ORDER BY id DESC LIMIT ?3",
        )?;
        let rows = st.query_map(params![chat, before, count], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    fn file_get(&mut self, id: i64) -> Result<Option<Blob>, StoreError> {
        self.one("SELECT v FROM files WHERE id = ?1", [id])
    }

    fn file_put(&mut self, id: i64, v: Blob) -> Result<(), StoreError> {
        self.write(
            "INSERT INTO files (id, v) VALUES (?1, ?2) ON CONFLICT (id) DO UPDATE SET v = excluded.v",
            params![id, v],
        )
        .map(drop)
    }

    fn file_delete(&mut self, id: i64) -> Result<(), StoreError> {
        self.write("DELETE FROM files WHERE id = ?1", [id])
            .map(drop)
    }

    fn seq_next(&mut self, name: &str) -> Result<i64, StoreError> {
        let mut st = self.conn.prepare_cached(
            "INSERT INTO seq (name, next) VALUES (?1, 1) \
             ON CONFLICT (name) DO UPDATE SET next = next + 1 RETURNING next",
        )?;
        Ok(st.query_row([name], |r| r.get(0))?)
    }

    fn counters(&mut self) -> Result<Marks, StoreError> {
        super::dump::counters(self)
    }

    fn dump(&mut self) -> Result<Tables, StoreError> {
        super::dump::dump(self)
    }
}
