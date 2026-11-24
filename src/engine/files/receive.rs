//! ReceiveFile and the download: the user's yes, once the whole description is here (Internal.hs `receiveFile'`, `receiveViaCompleteFD`; Subscriber.hs `processFDMessage`).

use crate::types::model::{ChatRef, FileState};

use super::body::media_of;
use super::transfers::{DownloadReq, FileKey};
use crate::engine::agent_port::AgentPort;
use crate::engine::error::{Code, EngineError, Result, fail};
use crate::engine::machine::Engine;
use crate::engine::records::file::{FileRecord, Way};
use crate::xftp::description::{FileDescription, Party};
use crate::xftp::{GB, MB};

/// `min(expected, 5 GiB) + 10 MiB` (`Internal.hs` `receiveFile'`): what a description may promise.
fn cap(size: u64) -> u64 {
    size.min(5 * GB) + 10 * MB
}

impl<A: AgentPort> Engine<A> {
    pub(crate) fn receive_file(&mut self, chat: ChatRef, item: i64) -> Result<()> {
        let (it, mut rec) = self.file_of(chat, item)?;
        let state = Self::file_state(&it, &rec);
        let may = rec.way == Way::Received
            && !rec.sender_cancelled
            && matches!(
                state,
                Some(FileState::Offered | FileState::Failed | FileState::Cancelled)
            );
        if !may {
            return Err(fail(
                Code::RequestFailed,
                "that file cannot be received now",
            ));
        }
        rec.accepted = true;
        if !rec.descr_complete {
            self.save_file(chat, item, &rec)?;
            return self.show_file(chat, item, &rec, FileState::Receiving { percent: 0 });
        }
        let size = media_of(&it.body).map_or(0, |m| m.size);
        self.begin_download(FileKey::of(chat, item), rec, size)
    }

    /// Starts the fetch of a complete description. A server the user's list does not know
    /// leaves the file offered (`relaysNotApproved`) and says so.
    pub(super) fn begin_download(
        &mut self,
        key: FileKey,
        mut rec: FileRecord,
        size: u64,
    ) -> Result<()> {
        let (chat, item) = (key.chat(), key.item);
        let refused = |this: &mut Self, mut rec: FileRecord, why: &str| {
            rec.accepted = false;
            let _ = this.save_file(chat, item, &rec); // the failure reported is the reason
            let _ = this.show_file(chat, item, &rec, FileState::Offered); // the same
            fail(Code::RequestFailed, why.to_owned())
        };
        let fd = match FileDescription::parse_valid(&rec.descr) {
            Ok(fd) if fd.redirect.is_some() => return Err(self.redirect_refused(key, rec)),
            Ok(fd) if fd.party == Party::Recipient && fd.size <= cap(size) => fd,
            Ok(_) => return Err(refused(self, rec, "file description does not fit the file")),
            Err(e) => return Err(refused(self, rec, &format!("bad file description: {e}"))),
        };
        let all_known =
            (fd.chunks.iter().flat_map(|c| &c.replicas)).all(|r| self.files.knows(&r.server));
        if !all_known {
            return Err(refused(self, rec, "file server not approved"));
        }
        if let Err(e) = self.room_for_transfer() {
            return Err(refused(self, rec, &e.detail));
        }
        let (job, cancel) = self.files.start(key);
        rec.active = true;
        self.save_file(chat, item, &rec)?;
        self.show_file(chat, item, &rec, FileState::Receiving { percent: 0 })?;
        let req = DownloadReq {
            key,
            job,
            description: fd,
            work_dir: self.files.dir.join("xftp-tmp"),
            dest_dir: self.files.dir.join("files"),
            max_size: cap(size),
            cancel,
        };
        self.files.transfers.download(req, self.jobs.tx.clone());
        Ok(())
    }

    /// `receiveViaCompleteFD`: "TODO re-enable redirects with relay checks". A redirect could
    /// send the download to servers the user never approved, so the file fails for good.
    fn redirect_refused(&mut self, key: FileKey, mut rec: FileRecord) -> EngineError {
        let (chat, item) = (key.chat(), key.item);
        rec.accepted = false;
        let _ = self.save_file(chat, item, &rec); // the failure reported is the reason
        let _ = self.show_file(chat, item, &rec, FileState::Failed); // the same
        fail(Code::RequestFailed, "redirect not allowed")
    }
}
