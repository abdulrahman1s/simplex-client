//! OpenFile and LocateFile: the copy on this computer, if it is still there.

use crate::types::Event;
use crate::types::model::ChatRef;

use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::machine::Engine;

impl<A: AgentPort> Engine<A> {
    /// The local copy of the file of an item: sent from here, or fetched, and not deleted since.
    fn local_copy(
        &mut self,
        chat: ChatRef,
        item: i64,
    ) -> Option<(std::path::PathBuf, crate::types::FileKind)> {
        let rec = self.file_record(chat, item).ok()??;
        let path = rec.local.filter(|p| self.files.store.exists(p))?;
        Some((path, rec.kind))
    }

    /// Where the file of an item is, when it is still here.
    pub(crate) fn local_file(&mut self, chat: ChatRef, item: i64) -> Option<std::path::PathBuf> {
        self.local_copy(chat, item).map(|(p, _)| p)
    }

    pub(crate) fn open_file(&mut self, chat: ChatRef, item: i64) -> Result<()> {
        let (path, kind) = self
            .local_copy(chat, item)
            .ok_or_else(|| fail(Code::RequestFailed, "that file is not on this computer"))?;
        self.emit(Event::OpenFile { path, kind });
        Ok(())
    }

    /// Never fails: answers `Located { path: None }` when the file is not here.
    pub(crate) fn locate_file(&mut self, chat: ChatRef, item: i64, tag: u64) -> Result<()> {
        let path = self.local_file(chat, item);
        self.emit(Event::Located { tag, path });
        Ok(())
    }
}
