//! Forward: a message goes to another chat as the user's own, marked forwarded (Commands.hs `APIForwardChatItems`, `prepareForward`, `mcForward`).

use std::path::Path;

use crate::types::FileKind;
use crate::types::model::{ChatRef, MessageBody};

use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, Result, fail};
use crate::engine::files::FileMeta;
use crate::engine::link::Outgoing;
use crate::engine::machine::Engine;

impl<A: AgentPort> Engine<A> {
    /// A file goes again when it is here; otherwise its caption goes as text (`contentWithoutFile`),
    /// and a file with no caption is not forwarded. Replies and mentions stay behind.
    pub(crate) fn forward(&mut self, from: ChatRef, item: i64, to: ChatRef) -> Result<()> {
        let it =
            (self.item(from, item)?).ok_or_else(|| fail(Code::RequestFailed, "no such item"))?;
        let forwarded = Outgoing {
            link: it.link.as_ref(),
            forward: true,
        };
        let (kind, media) = match &it.body {
            MessageBody::Text(t) => return self.send_text(to, t, None, &[], forwarded),
            MessageBody::Gif(p) => {
                let meta = file_meta(None, None, "");
                return self.send_file(to, Path::new(p), FileKind::Gif, meta);
            }
            MessageBody::Image(m) => (FileKind::Image, m),
            MessageBody::Video(m) => (FileKind::Video, m),
            MessageBody::Voice(m) => (FileKind::Voice, m),
            MessageBody::File(m) => (FileKind::File, m),
            MessageBody::Notice(_)
            | MessageBody::Call(_)
            | MessageBody::Stream(_)
            | MessageBody::Deleted => {
                return Err(fail(Code::RequestFailed, "nothing to forward"));
            }
        };
        if let Some(path) = self.local_file(from, item) {
            let meta = file_meta(media.preview.as_deref(), media.duration, &media.caption);
            return self.send_file(to, &path, kind, meta);
        }
        if media.caption.is_empty() {
            return Err(fail(Code::RequestFailed, "the file is not here to forward"));
        }
        let caption = media.caption.clone();
        self.send_text(to, &caption, None, &[], forwarded)
    }
}

const fn file_meta<'a>(
    preview: Option<&'a [u8]>,
    duration: Option<u32>,
    text: &'a str,
) -> FileMeta<'a> {
    FileMeta {
        preview,
        duration,
        text,
        forward: true,
    }
}
