//! The messages a tagged request sent, and their `Sent` or failure told against its tag.

use crate::types::Event;
use crate::types::model::SendStatus;

use crate::engine::agent_port::{AgentFailure, AgentMsgId, AgentPort, ConnId};
use crate::engine::files::FileKey;
use crate::engine::machine::Engine;

/// The tag one message is told against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SendTag {
    pub(crate) tag: u64,
    /// Only a failure is told: one part of a file's sending, whose `Sent` comes with the last.
    pub(crate) quiet: bool,
    /// The file it is part of, which then leaves out a recipient that failed.
    pub(crate) file: Option<FileKey>,
}

impl SendTag {
    pub(crate) const fn loud(tag: u64) -> Self {
        Self {
            tag,
            quiet: false,
            file: None,
        }
    }
}

impl<A: AgentPort> Engine<A> {
    /// `agent.send`, keeping the tag of the request being handled with the message.
    pub(crate) fn agent_send(
        &mut self,
        conn: &ConnId,
        pq: bool,
        body: Vec<u8>,
    ) -> Result<AgentMsgId, AgentFailure> {
        let id = self.agent.send(&mut self.store, conn, pq, body)?;
        if let Some(tag) = self.tagging {
            self.send_tags.insert((*conn, id), SendTag::loud(tag));
        }
        Ok(id)
    }

    /// The tag's status for one message; its entry goes with it.
    pub(crate) fn tag_outcome(&mut self, conn: &ConnId, msg: AgentMsgId, status: SendStatus) {
        let Some(t) = self.send_tags.remove(&(*conn, msg)) else {
            return;
        };
        let failed = matches!(status, SendStatus::Failed { .. });
        if let (true, Some(file)) = (failed, t.file) {
            // One word per recipient: the parts still going to them say nothing more, and
            // a description not sent yet is not tagged.
            self.send_tags
                .retain(|(c, _), o| !(c == conn && o.tag == t.tag && o.file == t.file));
            if let Some(f) = self.files.tags.get_mut(&file) {
                f.failed.insert(*conn);
            }
        }
        if t.quiet && !failed {
            return;
        }
        let member = self.state.member_of(conn).map(|(_, m)| m);
        self.emit(Event::SendStatus {
            tag: t.tag,
            member,
            status,
        });
    }
}
