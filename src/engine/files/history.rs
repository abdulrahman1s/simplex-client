//! An item's content and file as a group's history sends them again (Internal.hs `sendHistory` `itemForwardMsgs`, `getRcvFileInvDescr`, `getSndFileInvDescr`, `invCompleteDescr`).

use std::fs;
use std::path::Path;

use crate::types::FileKind;
use crate::types::model::{ChatRef, FileState, MessageBody};

use super::body::content;
use crate::chat::content::MsgContent;
use crate::chat::file::FileInvitation;
use crate::engine::agent_port::AgentPort;
use crate::engine::error::Result;
use crate::engine::machine::Engine;
use crate::engine::records::ItemRecord;
use crate::engine::records::file::FileRecord;

/// The agent's `rcvFilesTTL`: how long a file stays on its servers when the item names no
/// expiry, which this engine does not keep.
const FILE_TTL_MS: i64 = 2 * 24 * 3600 * 1000;

/// The offer and the whole description a new member fetches the file with.
pub(crate) struct HistoryFile {
    pub invitation: FileInvitation,
    pub descr: String,
}

/// A file's name, size and caption as its item shows them.
fn shown(body: &MessageBody) -> Option<(String, u64, &str)> {
    match body {
        MessageBody::Image(m)
        | MessageBody::Video(m)
        | MessageBody::File(m)
        | MessageBody::Voice(m) => Some((m.name.clone(), m.size, m.caption.as_str())),
        MessageBody::Gif(p) => {
            let path = Path::new(p);
            let name = path.file_name()?.to_string_lossy().into_owned();
            Some((name, fs::metadata(path).map_or(0, |m| m.len()), ""))
        }
        MessageBody::Text(_)
        | MessageBody::Notice(_)
        | MessageBody::Call(_)
        | MessageBody::Stream(_)
        | MessageBody::Deleted => None,
    }
}

fn kind_of(body: &MessageBody) -> FileKind {
    match body {
        MessageBody::Image(_) => FileKind::Image,
        MessageBody::Gif(_) => FileKind::Gif,
        MessageBody::Video(_) => FileKind::Video,
        MessageBody::Voice(_) => FileKind::Voice,
        MessageBody::File(_)
        | MessageBody::Text(_)
        | MessageBody::Notice(_)
        | MessageBody::Call(_)
        | MessageBody::Stream(_)
        | MessageBody::Deleted => FileKind::File,
    }
}

impl<A: AgentPort> Engine<A> {
    /// The content of a file item and its file, when the description is whole and the file
    /// neither cancelled nor expired; `None` for an item with no file.
    pub(crate) fn history_file(
        &mut self,
        chat: ChatRef,
        id: i64,
        it: &ItemRecord,
    ) -> Result<Option<(MsgContent, Option<HistoryFile>)>> {
        let Some((name, size, caption)) = shown(&it.body) else {
            return Ok(None);
        };
        let rec = self.file_record(chat, id)?;
        let kind = rec.as_ref().map_or_else(|| kind_of(&it.body), |r| r.kind);
        let media = super::body::media_of(&it.body);
        let preview = media.and_then(|m| m.preview.as_deref());
        let duration = media.and_then(|m| m.duration);
        let c = content(kind, caption, preview, duration);
        let cancelled = media.is_some_and(|m| m.state == FileState::Cancelled);
        let expired = self.clock.wall_ms() >= it.at.saturating_add(FILE_TTL_MS);
        let file = rec
            .filter(|r: &FileRecord| r.descr_complete && !r.sender_cancelled)
            .filter(|_| !cancelled && !expired)
            .map(|r| HistoryFile {
                invitation: FileInvitation::xftp(&name, size),
                descr: r.descr,
            });
        Ok(Some((c, file)))
    }
}
