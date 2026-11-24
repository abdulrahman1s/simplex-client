//! Files held in memory for a client without a file system: what JS hands over to send, and what downloads bring until JS takes them.
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use super::store::{FileStore, size_allowed};
use crate::engine::error::{Code, Result, fail};
use crate::xftp::{MB, safe_file_name};

/// The default largest file, either way: a browser tab's memory is the limit.
pub const DEFAULT_LIMIT: u64 = 64 * MB;
/// Files held at once; with the byte budget, the bound on what waits for JS.
const MAX_FILES: usize = 64;

struct Entry {
    name: String,
    bytes: Vec<u8>,
    /// A download's result, waiting for JS; the oldest go first when memory runs short.
    received: bool,
}

struct Inner {
    /// Eviction: a file to send leaves when its upload ends, a received one when JS takes it
    /// or, oldest first, when a new file needs the room. At most `MAX_FILES`, `4 × limit` bytes.
    files: BTreeMap<u64, Entry>,
    limit: u64,
    next: u64,
}

/// One store shared by the engine (as its `FileStore`) and the transfers.
#[derive(Clone)]
pub struct MemFiles(Arc<Mutex<Inner>>);

fn id_of(path: &Path) -> Option<u64> {
    let mut parts = path.components();
    let (Some(Component::RootDir), Some(Component::Normal(root))) = (parts.next(), parts.next())
    else {
        return None;
    };
    (root == "mem").then_some(())?;
    parts.next()?.as_os_str().to_str()?.parse().ok()
}

impl MemFiles {
    #[must_use]
    pub fn new(limit: u64) -> Self {
        Self(Arc::new(Mutex::new(Inner {
            files: BTreeMap::new(),
            limit,
            next: 0,
        })))
    }

    fn inner(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// The largest file sent or received.
    #[must_use]
    pub fn limit(&self) -> u64 {
        self.inner().limit
    }

    pub fn set_limit(&self, bytes: u64) {
        self.inner().limit = bytes;
    }

    /// Keeps `bytes` under a path of its own, `/mem/<n>/<name>`.
    pub fn put(&self, name: &str, bytes: Vec<u8>, received: bool) -> Result<PathBuf> {
        let mut g = self.inner();
        let size = bytes.len() as u64;
        if size > g.limit {
            return Err(fail(
                Code::RequestFailed,
                "file is larger than allowed in memory",
            ));
        }
        let budget = g.limit.saturating_mul(4);
        let held = |g: &Inner| g.files.values().map(|e| e.bytes.len() as u64).sum::<u64>();
        while g.files.len() >= MAX_FILES || held(&g) + size > budget {
            let Some(oldest) = g.files.iter().find(|(_, e)| e.received).map(|(k, _)| *k) else {
                return Err(fail(Code::RequestFailed, "too many files in memory"));
            };
            g.files.remove(&oldest);
        }
        g.next += 1;
        let (id, name) = (g.next, safe_file_name(name));
        let path = PathBuf::from(format!("/mem/{id}/{name}"));
        g.files.insert(
            id,
            Entry {
                name,
                bytes,
                received,
            },
        );
        Ok(path)
    }

    /// Runs `f` on the bytes of `path` while they are held.
    pub fn with_bytes<R>(&self, path: &Path, f: impl FnOnce(&[u8]) -> R) -> Option<R> {
        let g = self.inner();
        g.files.get(&id_of(path)?).map(|e| f(&e.bytes))
    }

    /// Hands the file over: its name and bytes, no longer held here.
    pub fn take(&self, path: &Path) -> Option<(String, Vec<u8>)> {
        let e = self.inner().files.remove(&id_of(path)?)?;
        Some((e.name, e.bytes))
    }
}

impl FileStore for MemFiles {
    fn source(&self, _dir: &Path, path: &Path) -> Result<(PathBuf, u64)> {
        let not_found = || fail(Code::RequestFailed, "file not found");
        let size = self
            .with_bytes(path, |b| b.len() as u64)
            .ok_or_else(not_found)?;
        size_allowed(size)?;
        Ok((path.to_path_buf(), size))
    }

    fn exists(&self, path: &Path) -> bool {
        self.with_bytes(path, |_| ()).is_some()
    }

    fn remove(&self, _dir: &Path, path: &Path) -> std::result::Result<(), String> {
        if let Some(id) = id_of(path) {
            self.inner().files.remove(&id);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn received_files_make_room_oldest_first_and_files_to_send_never_do() {
        let mem = MemFiles::new(100);
        let send = mem.put("a.jpg", vec![1; 100], false).unwrap();
        let old = mem.put("b.jpg", vec![2; 100], true).unwrap();
        let new = mem.put("c.jpg", vec![3; 100], true).unwrap();
        mem.put("d.jpg", vec![4; 100], true).unwrap();
        mem.put("e.jpg", vec![5; 100], true).unwrap();
        assert!(!mem.exists(&old), "the oldest received file made room");
        assert!(mem.exists(&send) && mem.exists(&new));
        assert!(mem.put("f", vec![0; 101], false).is_err(), "over the limit");
        assert_eq!(mem.take(&new), Some(("c.jpg".to_owned(), vec![3; 100])));
        assert!(!mem.exists(&new), "taken is gone");
    }

    #[test]
    fn a_memory_path_names_its_file_and_nothing_else() {
        assert_eq!(MemFiles::new(DEFAULT_LIMIT).limit(), 64 << 20);
        let mem = MemFiles::new(10);
        let p = mem.put("../x/y.txt", vec![1], false).unwrap();
        assert_eq!(p.file_name().unwrap(), "y.txt");
        assert_eq!(mem.source(Path::new(""), &p).unwrap(), (p.clone(), 1));
        assert!(!mem.exists(Path::new("/etc/passwd")));
        assert!(!mem.exists(Path::new("/mem/99/y.txt")));
        mem.remove(Path::new(""), &p).unwrap();
        assert!(mem.source(Path::new(""), &p).is_err());
    }
}
