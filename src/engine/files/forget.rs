//! A removed friend's files: transfers stopped, local copies deleted (Commands.hs
//! `APIDeleteChat` → `deleteFilesLocally`, `cancelFilesInProgress`).

use std::sync::atomic::Ordering;

use crate::types::model::ChatRef;

use super::transfers::{FileKey, owner};
use crate::engine::agent_port::AgentPort;
use crate::engine::error::Result;
use crate::engine::machine::Engine;
use crate::engine::records::file::FileRecord;
use crate::engine::records::{NS, decode, keys};

impl<A: AgentPort> Engine<A> {
    /// Before the friend's records go: their records stay for the caller to delete. A group's
    /// files go the same way by its `files::owner` number.
    pub(crate) fn forget_files(&mut self, friend: i64) -> Result<()> {
        let running: Vec<_> = self
            .files
            .active
            .keys()
            .filter(|k| k.friend == friend)
            .copied()
            .collect();
        for key in running {
            if let Some(a) = self.files.active.remove(&key) {
                a.cancel.store(true, Ordering::Relaxed);
            }
        }
        for key in self
            .store
            .keys_with_prefix(NS, &keys::file_prefix(friend))?
        {
            let Some(bytes) = self.store.get(NS, &key)? else {
                continue;
            };
            let rec: FileRecord = decode(&bytes)?;
            // Only copies in the engine's own place: a path outside it is never ours to delete.
            if let Some(p) = rec.local
                && let Err(e) = self.files.store.remove(&self.files.dir, &p)
            {
                self.log.line(format_args!("a file was not deleted: {e}"));
            }
        }
        Ok(())
    }

    /// One item's file, as its item goes for good (Internal.hs `deleteCIFiles`): its transfer
    /// stopped, the engine's own copy deleted, its record gone.
    pub(crate) fn drop_item_file(&mut self, chat: ChatRef, item: i64) -> Result<()> {
        let Some(rec) = self.file_record(chat, item)? else {
            return Ok(());
        };
        self.stop_transfer(FileKey::of(chat, item));
        if let Some(p) = rec.local
            && let Err(e) = self.files.store.remove(&self.files.dir, &p)
        {
            self.log.line(format_args!("a file was not deleted: {e}"));
        }
        self.store.delete(NS, &keys::file(owner(chat), item))?;
        Ok(())
    }
}
