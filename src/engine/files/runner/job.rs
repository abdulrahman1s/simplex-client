//! One transfer in the runner: its machine, the command in hand and the one connection it holds.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crossbeam_channel::Sender;

use super::super::mem_files::MemFiles;
use super::super::outcome::{Throttle, recipients_for, upload_done};
use super::super::queued::FileJob;
use super::super::transfers::{FileDone, FileKey};
use super::link::FileLink;
use super::work::{Out, Work};
use crate::engine::jobs::JobDone;
use crate::xftp::{Call, MemDownload, MemUpload, XftpError};

pub(super) struct Job {
    key: FileKey,
    job: u64,
    cancel: Arc<AtomicBool>,
    pub(super) tx: Sender<JobDone>,
    pub(super) work: Work,
    /// The command in hand, and whether it went out on the link.
    pub(super) call: Option<(Box<Call>, bool)>,
    /// A failure the machine reported; the job ends with it.
    pub(super) failed: Option<XftpError>,
    pub(super) link: Option<FileLink>,
    /// No dial before this, after connections failed.
    pub(super) retry_at: u64,
    pub(super) failed_dials: u32,
    pub(super) throttle: Throttle,
}

fn upload(req: &super::super::transfers::UploadReq, mem: &MemFiles) -> Result<Work, XftpError> {
    let recipients = recipients_for(req.members);
    let made = mem.with_bytes(&req.source, |b| {
        MemUpload::new(b, &req.name, &req.servers, recipients, None)
    });
    let machine =
        made.unwrap_or_else(|| Err(XftpError::File("the file is not in memory".into())))?;
    Ok(Work::Upload {
        machine: Box::new(machine),
        members: req.members,
        source: req.source.clone(),
    })
}

impl Job {
    /// The job, or at once its result when the transfer cannot start.
    pub(super) fn start(
        job: FileJob,
        tx: Sender<JobDone>,
        mem: &MemFiles,
    ) -> Result<Self, FileDone> {
        let (key, run, cancel, work) = match job {
            FileJob::Upload(req) => match upload(&req, mem) {
                Ok(work) => (req.key, req.job, req.cancel, work),
                Err(e) => {
                    mem.take(&req.source); // nothing will send it now
                    return Err(upload_done(
                        req.key,
                        req.job,
                        req.members,
                        Err(e.to_string()),
                    ));
                }
            },
            FileJob::Download(req) => {
                match MemDownload::new(&req.description, mem.limit().min(req.max_size)) {
                    Ok(m) => (req.key, req.job, req.cancel, Work::Download(Box::new(m))),
                    Err(e) => {
                        let (key, job, result) = (req.key, req.job, Err(e.to_string()));
                        return Err(FileDone::Downloaded { key, job, result });
                    }
                }
            }
        };
        Ok(Self {
            key,
            job: run,
            cancel,
            tx,
            work,
            call: None,
            failed: None,
            link: None,
            retry_at: 0,
            failed_dials: 0,
            throttle: Throttle::new(key, run),
        })
    }

    pub(super) fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    /// The job's result for the engine; its link closes, and its file leaves memory or arrives.
    pub(super) fn finish(&mut self, out: Result<Out, XftpError>, mem: &MemFiles) -> FileDone {
        if let Some(link) = self.link.take() {
            link.close();
        }
        let (key, job) = (self.key, self.job);
        match (&self.work, out) {
            (
                Work::Upload {
                    members, source, ..
                },
                out,
            ) => {
                mem.take(source);
                let up = match out {
                    Ok(Out::Uploaded(u)) => Ok(u),
                    Ok(Out::Fetched(_)) => Err("a download's result".to_owned()),
                    Err(e) => Err(e.to_string()),
                };
                upload_done(key, job, *members, up)
            }
            (Work::Download(_), out) => {
                let result = match out {
                    Ok(Out::Fetched(f)) => {
                        (mem.put(&f.file_name, f.bytes, true)).map_err(|e| e.detail)
                    }
                    Ok(Out::Uploaded(_)) => Err("an upload's result".to_owned()),
                    Err(e) => Err(e.to_string()),
                };
                FileDone::Downloaded { key, job, result }
            }
        }
    }
}
