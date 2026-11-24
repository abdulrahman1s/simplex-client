//! Where the engine's file copies live: the data folder natively, memory in a browser (Commands.hs `checkSndFile`, `deleteFilesLocally`).
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::engine::error::{Code, Result, fail};
use crate::limits::MAX_FILE_SIZE;

/// The engine's view of its local copies; a copy is named by its path either way.
pub trait FileStore: Send {
    /// The file `path` names, as the engine may send it: its canonical path and its size.
    fn source(&self, dir: &Path, path: &Path) -> Result<(PathBuf, u64)>;
    /// Whether the copy is still there.
    fn exists(&self, path: &Path) -> bool;
    /// Deletes a copy of the engine's own; one outside `dir` is never touched. Gone already is fine.
    fn remove(&self, dir: &Path, path: &Path) -> std::result::Result<(), String>;
}

/// What a sent file must be: not empty, within XFTP's limit (Commands.hs `checkSndFile`).
pub(super) fn size_allowed(size: u64) -> Result<()> {
    if size == 0 {
        return Err(fail(Code::RequestFailed, "the file is empty"));
    }
    if size > MAX_FILE_SIZE {
        return Err(fail(Code::FileTooLarge, "the file is over 1 GiB"));
    }
    Ok(())
}

/// Files in the data folder.
#[derive(Debug, Default)]
pub struct DiskFiles;

impl FileStore for DiskFiles {
    /// Inside the data folder (a path from outside is never read), a regular file.
    fn source(&self, dir: &Path, path: &Path) -> Result<(PathBuf, u64)> {
        let own = fs::canonicalize(dir).map_err(|_| fail(Code::RequestFailed, "no data folder"))?;
        let file =
            fs::canonicalize(path).map_err(|_| fail(Code::RequestFailed, "file not found"))?;
        if !file.starts_with(&own) {
            return Err(fail(Code::RequestFailed, "file is outside the data folder"));
        }
        let meta = fs::metadata(&file).map_err(|_| fail(Code::RequestFailed, "file not found"))?;
        if !meta.is_file() {
            return Err(fail(Code::RequestFailed, "file size is not allowed"));
        }
        size_allowed(meta.len())?;
        Ok((file, meta.len()))
    }

    fn exists(&self, path: &Path) -> bool {
        path.is_file()
    }

    /// A sent file's path is canonical, a fresh download's is under `dir` as given; either counts.
    fn remove(&self, dir: &Path, path: &Path) -> std::result::Result<(), String> {
        let canonical = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
        let ours = path.starts_with(dir) || path.starts_with(&canonical);
        if dir.as_os_str().is_empty() || !ours {
            return Ok(());
        }
        match fs::remove_file(path) {
            Err(e) if e.kind() != ErrorKind::NotFound => Err(e.to_string()),
            Ok(()) | Err(_) => Ok(()),
        }
    }
}
