//! Transfers handed to a runner outside the engine, for a client without threads: each request with the channel its result goes to.
use crossbeam_channel::{Receiver, Sender, unbounded};

use super::transfers::{DownloadReq, FileDone, Transfers, UploadReq};
use crate::engine::jobs::JobDone;

pub enum FileJob {
    Upload(UploadReq),
    Download(DownloadReq),
}

/// The engine's end; the runner holds the receiver.
pub struct QueuedTransfers {
    tx: Sender<(FileJob, Sender<JobDone>)>,
}

/// Unbounded, but bounded in practice: the engine starts at most `files::MAX_ACTIVE` transfers.
#[must_use]
pub fn queue() -> (QueuedTransfers, Receiver<(FileJob, Sender<JobDone>)>) {
    let (tx, rx) = unbounded();
    (QueuedTransfers { tx }, rx)
}

impl Transfers for QueuedTransfers {
    fn upload(&mut self, req: UploadReq, tx: Sender<JobDone>) {
        let (key, job) = (req.key, req.job);
        if self.tx.send((FileJob::Upload(req), tx.clone())).is_err() {
            let result = Err("no file runner".to_owned());
            let _ = tx.send(JobDone::File(FileDone::Uploaded { key, job, result })); // the engine is gone
        }
    }

    fn download(&mut self, req: DownloadReq, tx: Sender<JobDone>) {
        let (key, job) = (req.key, req.job);
        if self.tx.send((FileJob::Download(req), tx.clone())).is_err() {
            let result = Err("no file runner".to_owned());
            let _ = tx.send(JobDone::File(FileDone::Downloaded { key, job, result })); // the engine is gone
        }
    }
}
