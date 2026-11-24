//! The XFTP transfers on two worker threads, a cancel flag per job and progress in 5% steps.

use std::fs;
use std::thread;

use crossbeam_channel::{Receiver, Sender, unbounded};

use super::outcome::{Throttle, recipients_for, upload_done};
use super::transfers::{DownloadReq, FileDone, Transfers, UploadReq};
use crate::engine::jobs::JobDone;
use crate::xftp::{DownloadParams, UploadParams, Uploaded, download, upload};

/// Threads that move files; the rest of the transfers wait in the queue.
const WORKERS: usize = 2;

type Work = Box<dyn FnOnce() + Send>;

pub struct XftpTransfers {
    queue: Option<Sender<Work>>,
}

impl XftpTransfers {
    #[must_use]
    pub fn new() -> Self {
        Self { queue: None }
    }

    /// Threads start with the first transfer, so an engine that never moves a file has none.
    /// The queue is bounded by the engine, which starts at most `files::MAX_ACTIVE` jobs.
    fn submit(&mut self, work: Work) -> Result<(), String> {
        let tx = self.queue.get_or_insert_with(|| {
            let (tx, rx) = unbounded::<Work>();
            for n in 0..WORKERS {
                spawn(n, rx.clone());
            }
            tx
        });
        tx.send(work).map_err(|_| "no transfer threads".to_owned())
    }
}

impl Default for XftpTransfers {
    fn default() -> Self {
        Self::new()
    }
}

fn spawn(n: usize, rx: Receiver<Work>) {
    let started = thread::Builder::new()
        .name(format!("simplex-xftp-{n}"))
        .spawn(move || rx.iter().for_each(|work| work()));
    // A thread that cannot start leaves the other to do the work, or `submit` to fail.
    drop(started);
}

impl Transfers for XftpTransfers {
    fn upload(&mut self, req: UploadReq, tx: Sender<JobDone>) {
        let (key, job) = (req.key, req.job);
        let failed = tx.clone();
        let work: Work = Box::new(move || {
            let up = run_upload(&req, &tx);
            let _ = tx.send(JobDone::File(upload_done(key, job, req.members, up))); // the engine is gone
        });
        if let Err(e) = self.submit(work) {
            let _ = failed.send(JobDone::File(FileDone::Uploaded {
                key,
                job,
                result: Err(e),
            })); // the engine is gone
        }
    }

    fn download(&mut self, req: DownloadReq, tx: Sender<JobDone>) {
        let (key, job) = (req.key, req.job);
        let failed = tx.clone();
        let work: Work = Box::new(move || {
            let result = run_download(&req, &tx);
            let _ = tx.send(JobDone::File(FileDone::Downloaded { key, job, result })); // the engine is gone
        });
        if let Err(e) = self.submit(work) {
            let _ = failed.send(JobDone::File(FileDone::Downloaded {
                key,
                job,
                result: Err(e),
            })); // the engine is gone
        }
    }
}

fn run_upload(req: &UploadReq, tx: &Sender<JobDone>) -> Result<Uploaded, String> {
    fs::create_dir_all(&req.work_dir).map_err(|e| e.to_string())?;
    let params = UploadParams {
        source: &req.source,
        file_name: &req.name,
        work_dir: &req.work_dir,
        servers: &req.servers,
        recipients: recipients_for(req.members),
        storage_hours: None,
    };
    let mut throttle = Throttle::new(req.key, req.job);
    let mut progress = |done, total| throttle.tell(done, total, tx);
    upload(&params, &req.cancel, &mut progress).map_err(|e| e.to_string())
}

fn run_download(req: &DownloadReq, tx: &Sender<JobDone>) -> Result<std::path::PathBuf, String> {
    fs::create_dir_all(&req.work_dir).map_err(|e| e.to_string())?;
    fs::create_dir_all(&req.dest_dir).map_err(|e| e.to_string())?;
    let params = DownloadParams {
        description: &req.description,
        work_dir: &req.work_dir,
        dest_dir: &req.dest_dir,
        max_size: req.max_size,
    };
    let mut throttle = Throttle::new(req.key, req.job);
    let mut progress = |done, total| throttle.tell(done, total, tx);
    let got = download(&params, &req.cancel, &mut progress).map_err(|e| e.to_string())?;
    Ok(got.path)
}
