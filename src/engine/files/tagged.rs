//! A tagged file send told once per recipient: `Sent` with the last part of their description, `Failed` when their offer or a part fails or the upload ends without going out.

use std::collections::HashSet;

use crate::types::Event;
use crate::types::model::SendStatus;

use super::FileTag;
use super::transfers::FileKey;
use crate::engine::agent_port::{AgentMsgId, AgentPort, ConnId};
use crate::engine::machine::Engine;
use crate::engine::send_tags::SendTag;

impl<A: AgentPort> Engine<A> {
    /// The offer went: its messages tell only a failure, and the tag waits for the upload.
    pub(super) fn tag_file(&mut self, key: FileKey, offers: &[(i64, ConnId, AgentMsgId)]) {
        let Some(tag) = self.tagging else {
            return;
        };
        for (_, conn, id) in offers {
            if let Some(t) = self.send_tags.get_mut(&(*conn, *id)) {
                t.quiet = true;
                t.file = Some(key);
            }
        }
        let failed = HashSet::new();
        self.files.tags.insert(key, FileTag { tag, failed });
    }

    /// A recipient's description went in `parts`: the last tells `Sent`. One whose offer
    /// failed has been told already.
    pub(super) fn tag_description(
        &mut self,
        key: FileKey,
        tag: Option<&FileTag>,
        conn: ConnId,
        parts: &[AgentMsgId],
    ) {
        let Some(f) = tag.filter(|f| !f.failed.contains(&conn)) else {
            return;
        };
        let last = parts.len().saturating_sub(1);
        for (n, id) in parts.iter().enumerate() {
            let t = SendTag {
                tag: f.tag,
                quiet: n != last,
                file: Some(key),
            };
            self.send_tags.insert((conn, *id), t);
        }
    }

    /// The upload ended without the descriptions going: `Failed` for everyone, and the
    /// offers still going say nothing more.
    pub(super) fn file_failed(&mut self, key: FileKey, tag: u64, detail: &str) {
        self.send_tags.retain(|_, t| t.file != Some(key));
        self.emit(Event::SendStatus {
            tag,
            member: None,
            status: SendStatus::Failed {
                detail: detail.to_owned(),
            },
        });
    }
}
