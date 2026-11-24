//! What the engine's events mean for files: `fileProgress`, and the `sendFile` and `download` calls they settle.
use std::path::Path;

use serde_json::json;
use wasm_bindgen::JsValue;

use super::convert::{error, failure};
use super::files::{Files, Parts};
use super::outcome::{Outcome, Settled};
use super::payload::Payload;
use crate::engine::{Engine, NativeAgent};
use crate::types::model::{ChatMessage, ChatRef, FileState, MessageBody};
use crate::types::{ErrorCode, Event, RequestKind};

/// The file of a message: where it is here and how far it got. A GIF that is here is its path.
fn file_of(m: &ChatMessage) -> Option<(Option<&Path>, FileState)> {
    match &m.body {
        MessageBody::Image(f) | MessageBody::Video(f) | MessageBody::File(f) => {
            Some((f.local.as_deref(), f.state.clone()))
        }
        MessageBody::Gif(p) => Some((Some(Path::new(p)), FileState::Received(p.into()))),
        MessageBody::Text(_)
        | MessageBody::Notice(_)
        | MessageBody::Voice { .. }
        | MessageBody::Call(_)
        | MessageBody::Stream(_)
        | MessageBody::Deleted => None,
    }
}

/// Settles a `download` now: its bytes are a `Uint8Array`, which a JSON payload cannot carry.
pub(super) fn settle_now((resolve, reject): Parts, result: Result<JsValue, JsValue>) {
    let (f, v) = match &result {
        Ok(v) => (resolve, v),
        Err(v) => (reject, v),
    };
    let _ = f.call1(&JsValue::NULL, v); // a promise's own functions do not throw
}

/// `{name, bytes}` with the bytes as a `Uint8Array`.
pub(super) fn named(name: &str, bytes: &[u8]) -> JsValue {
    let o = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&o, &"name".into(), &name.into()); // a plain object takes any key
    let _ = js_sys::Reflect::set(&o, &"bytes".into(), &js_sys::Uint8Array::from(bytes).into());
    o.into()
}

impl Files {
    pub(super) fn hear(&mut self, e: &Event, out: &mut Outcome, engine: &mut Engine<NativeAgent>) {
        if let Event::Messages(list) = e {
            list.iter().for_each(|m| self.sent(m, out));
        }
        if let Event::Updated(list) = e {
            for m in list {
                self.progress(m, out);
                self.arrived(m);
            }
        }
        if let Event::Failed {
            request,
            code,
            detail,
        } = e
        {
            self.refused(*request, (*code, detail), out, engine);
        }
    }

    /// A `sendFile` resolves with the message its file went out with.
    fn sent(&mut self, m: &ChatMessage, out: &mut Outcome) {
        let Some((Some(local), _)) = file_of(m) else {
            return;
        };
        let Some(at) = self.sending.iter().position(|s| s.path == local) else {
            return;
        };
        if let Some(s) = self.sending.remove(at) {
            let (resolve, reject) = s.parts;
            let result = Ok(Payload::Message(Box::new(m.clone())));
            out.settled.push(Settled {
                resolve,
                reject,
                result,
            });
        }
    }

    fn progress(&mut self, m: &ChatMessage, out: &mut Outcome) {
        let (sending, percent) = match file_of(m) {
            Some((_, FileState::Sending { percent })) => (true, percent),
            Some((_, FileState::Receiving { percent })) => (false, percent),
            Some(_) | None => return,
        };
        let waiting = self.receiving.iter_mut();
        waiting
            .filter(|r| (r.chat, r.item) == (m.chat, m.id))
            .for_each(|r| r.started = true);
        let (kind, chat) = match m.chat {
            ChatRef::Direct(id) => ("friend", id),
            ChatRef::Group(id) => ("group", id),
        };
        let info = json!({ kind: chat, "id": m.id, "percent": percent, "sending": sending });
        out.calls.push(("fileProgress", Payload::Json(info)));
    }

    /// A `download` resolves with `{name, bytes}` once the file is here; they leave memory then.
    /// Only a waiting call takes them: other news of the message leaves memory alone.
    fn arrived(&mut self, m: &ChatMessage) {
        let Some((_, state)) = file_of(m) else { return };
        let at = (self.receiving.iter()).position(|r| (r.chat, r.item) == (m.chat, m.id));
        let Some(at) = at else { return };
        let result = match state {
            FileState::Received(path) => match self.mem.take(&path) {
                Some((name, bytes)) => Ok(named(&name, &bytes)),
                None => Err(error("file_gone", "the file is no longer in memory")),
            },
            FileState::Failed => Err(error("file_failed", "the download failed")),
            FileState::Cancelled => Err(error("file_cancelled", "the file was cancelled")),
            FileState::Offered
            | FileState::Receiving { .. }
            | FileState::Sending { .. }
            | FileState::Sent => return,
        };
        settle_now(self.receiving.remove(at).parts, result);
    }

    /// The engine refused a file request. Requests are handled in order, so a refused send is
    /// the oldest waiting; a refused download is the oldest whose file the engine did not take
    /// on (a failed cancel or locate shares the kind, but leaves a started download alone).
    fn refused(
        &mut self,
        request: RequestKind,
        (code, detail): (ErrorCode, &str),
        out: &mut Outcome,
        engine: &mut Engine<NativeAgent>,
    ) {
        if request == RequestKind::SendFile
            && let Some(s) = self.sending.pop_front()
        {
            self.mem.take(&s.path);
            let (resolve, reject) = s.parts;
            let result = Err(failure(code.name(), detail));
            out.settled.push(Settled {
                resolve,
                reject,
                result,
            });
        } else if request == RequestKind::File
            && let Some(at) = (self.receiving.iter())
                .position(|r| !r.started && !engine.file_wanted(r.chat, r.item))
        {
            let why = error(code.name(), detail);
            settle_now(self.receiving.remove(at).parts, Err(why));
        }
    }
}
