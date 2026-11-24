//! The file methods of the JS objects: `friend.sendFile`, `group.sendFile`, `message.download`.
use std::rc::Rc;

use wasm_bindgen::prelude::*;

use super::client::Client;
use super::convert::{closed, error};
use super::files::{Parts, Receiving, Sending};
use super::files_events::{named, settle_now};
use super::objects::{Friend, Group, Message};
use super::shared::Shared;
use crate::types::model::ChatRef;
use crate::types::{FileKind, Request};

fn promise() -> (js_sys::Promise, Option<Parts>) {
    let mut parts = None;
    let promise = js_sys::Promise::new(&mut |resolve, reject| parts = Some((resolve, reject)));
    (promise, parts)
}

fn kind(kind: Option<String>) -> Result<FileKind, JsValue> {
    let Some(kind) = kind else {
        return Ok(FileKind::File);
    };
    serde_json::from_value(serde_json::Value::String(kind)).map_err(|_| {
        error(
            "bad_argument",
            "kind is \"image\", \"gif\", \"video\" or \"file\"",
        )
    })
}

/// Keeps `bytes` in memory until the upload ends and asks the engine to send them; resolves
/// with the message the file goes out with.
fn send_file(
    shared: &Rc<Shared>,
    chat: ChatRef,
    bytes: Vec<u8>,
    name: &str,
    kind_name: Option<String>,
) -> js_sys::Promise {
    let (promise, parts) = promise();
    let queued = (|| {
        let kind = kind(kind_name)?;
        let parts = parts.ok_or_else(closed)?;
        let path = {
            let mut core = shared.core.borrow_mut();
            let files = &mut core.as_mut().ok_or_else(closed)?.files;
            let path =
                (files.mem.put(name, bytes, false)).map_err(|e| error(e.code.name(), e.detail))?;
            files.sending.push_back(Sending {
                path: path.clone(),
                parts,
            });
            path
        };
        let (preview, duration, text) = (None, None, String::new());
        shared.tell(Request::SendFile {
            chat,
            path,
            kind,
            preview,
            duration,
            text,
        })
    })();
    match queued {
        Ok(()) => promise,
        Err(e) => js_sys::Promise::reject(&e),
    }
}

#[wasm_bindgen]
impl Friend {
    /// Sends `bytes` as a file named `name`, a `kind` of `"image"`, `"gif"`, `"video"` or
    /// `"file"` (the default). Resolves with the message; `fileProgress` follows the upload.
    #[wasm_bindgen(js_name = sendFile, unchecked_return_type = "Promise<Message>")]
    pub fn send_file(&self, bytes: Vec<u8>, name: &str, kind: Option<String>) -> js_sys::Promise {
        send_file(self.shared(), self.chat(), bytes, name, kind)
    }
}

#[wasm_bindgen]
impl Group {
    /// Sends `bytes` as a file to every member; as `Friend.sendFile`.
    #[wasm_bindgen(js_name = sendFile, unchecked_return_type = "Promise<Message>")]
    pub fn send_file(&self, bytes: Vec<u8>, name: &str, kind: Option<String>) -> js_sys::Promise {
        send_file(self.shared(), self.chat(), bytes, name, kind)
    }
}

#[wasm_bindgen]
impl Message {
    /// The message's file as `{name, bytes}`, received first if it is not here yet; the
    /// bytes then leave the client's memory. `fileProgress` follows the download.
    #[wasm_bindgen(unchecked_return_type = "Promise<{ name: string, bytes: Uint8Array }>")]
    pub fn download(&self) -> js_sys::Promise {
        let (promise, parts) = promise();
        let Some(parts) = parts else {
            return js_sys::Promise::reject(&closed());
        };
        let (chat, item) = (self.info().chat, self.info().id);
        {
            let mut core = self.shared().core.borrow_mut();
            let Some(core) = core.as_mut() else {
                return js_sys::Promise::reject(&closed());
            };
            let here = core.engine.local_file(chat, item);
            if let Some((name, bytes)) = here.and_then(|p| core.files.mem.take(&p)) {
                settle_now(parts, Ok(named(&name, &bytes)));
                return promise;
            }
            let started = false;
            core.files.receiving.push(Receiving {
                chat,
                item,
                started,
                parts,
            });
        }
        match self.shared().tell(Request::ReceiveFile { chat, item }) {
            Ok(()) => promise,
            Err(e) => js_sys::Promise::reject(&e),
        }
    }
}

#[wasm_bindgen]
impl Client {
    /// The largest file sent or received, in bytes; 64 MiB unless set, and never above the
    /// official client's 1 GiB (`limits::MAX_FILE_SIZE`). Memory holds at most four times this
    /// in files waiting to go or to be taken.
    #[wasm_bindgen(js_name = setFileLimit)]
    pub fn set_file_limit(&self, bytes: f64) -> Result<(), JsValue> {
        if !(bytes.is_finite() && bytes >= 0.0) {
            return Err(error("bad_argument", "the limit is a number of bytes"));
        }
        let core = self.shared.core.borrow();
        let files = &core.as_ref().ok_or_else(closed)?.files;
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "checked finite and not negative"
        )]
        files.mem.set_limit(bytes as u64);
        Ok(())
    }
}
