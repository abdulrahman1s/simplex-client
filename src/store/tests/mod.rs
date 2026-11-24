//! Store rules, each run on both backends: SQLite in a scratch folder, and memory saved by snapshot.
mod atomic;
mod changes;
#[cfg(feature = "native")]
mod disk;
mod items;
mod kdf;
#[cfg(feature = "native")]
mod pragmas;
mod reopen;
mod snapshot;
mod tamper;
#[cfg(feature = "native")]
mod tamper_file;

use super::{Store, StoreError};

#[derive(Clone, Copy, Debug)]
pub(super) enum Kind {
    #[cfg(feature = "native")]
    Sqlite,
    Memory,
}

/// Where a test's store lives between opens: a SQLite file, or the last snapshot.
pub(super) struct Bed {
    pub kind: Kind,
    #[cfg(feature = "native")]
    pub dir: Dir,
    pub saved: Option<Vec<u8>>,
}

impl Bed {
    pub(super) fn new(kind: Kind) -> Bed {
        Bed {
            kind,
            #[cfg(feature = "native")]
            dir: Dir::new(),
            saved: None,
        }
    }

    pub(super) fn open(&self, key: &str) -> Result<Store, StoreError> {
        match self.kind {
            #[cfg(feature = "native")]
            Kind::Sqlite => Store::open(&self.dir.db(), key),
            Kind::Memory => match &self.saved {
                Some(bytes) => Store::open_snapshot(bytes, key),
                None => Store::open_memory(key),
            },
        }
    }

    pub(super) fn store(&self, key: &str) -> Store {
        self.open(key).expect("open store")
    }

    /// Ends this run of the store; the next `open` sees what it wrote.
    pub(super) fn close(&mut self, mut s: Store) {
        if matches!(self.kind, Kind::Memory) {
            self.saved = Some(s.export_snapshot().expect("snapshot"));
        }
    }
}

/// One test per backend for each rule: `both!(rule)` runs `fn rule(Bed)` as `rule::sqlite`
/// and `rule::memory`.
macro_rules! both {
    ($($rule:ident),* $(,)?) => {$(
        mod $rule {
            #[cfg(feature = "native")]
            #[test]
            fn sqlite() {
                super::$rule(super::super::Bed::new(super::super::Kind::Sqlite));
            }

            #[test]
            fn memory() {
                super::$rule(super::super::Bed::new(super::super::Kind::Memory));
            }
        }
    )*};
}
pub(super) use both;

/// A scratch directory removed on drop.
#[cfg(feature = "native")]
pub(super) struct Dir(pub std::path::PathBuf);

#[cfg(feature = "native")]
impl Dir {
    pub(super) fn new() -> Dir {
        let mut id = [0u8; 8];
        getrandom::fill(&mut id).expect("os randomness");
        let name: String = id.iter().map(|b| format!("{b:02x}")).collect();
        let dir = std::env::temp_dir().join(format!("sxc-store-{name}"));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        Dir(dir)
    }

    pub(super) fn db(&self) -> std::path::PathBuf {
        self.0.join("chat.db")
    }
}

#[cfg(feature = "native")]
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
