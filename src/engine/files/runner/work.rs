//! The transfer a job runs: an in-memory upload or download, behind one face.
use std::path::PathBuf;

use crate::h2::Progress;
use crate::xftp::{Answer, BLOCK_SIZE, Fetched, MemDownload, MemUpload, Step, Uploaded, XftpError};

pub(super) enum Work {
    Upload {
        machine: Box<MemUpload>,
        /// A group's members, one description each; `None` for a friend's file.
        members: Option<usize>,
        /// The file JS handed over; it leaves memory when the upload ends.
        source: PathBuf,
    },
    Download(Box<MemDownload>),
}

pub(super) enum Out {
    Uploaded(Uploaded),
    Fetched(Fetched),
}

impl Work {
    pub(super) fn next_step(&mut self) -> Result<Step<Out>, XftpError> {
        Ok(match self {
            Self::Upload { machine, .. } => match machine.next_step()? {
                Step::Call(c) => Step::Call(c),
                Step::Done(u) => Step::Done(Out::Uploaded(u)),
            },
            Self::Download(machine) => match machine.next_step()? {
                Step::Call(c) => Step::Call(c),
                Step::Done(f) => Step::Done(Out::Fetched(f)),
            },
        })
    }

    pub(super) fn on_answer(&mut self, answer: Answer) -> Result<(), XftpError> {
        match self {
            Self::Upload { machine, .. } => machine.on_answer(answer),
            Self::Download(machine) => machine.on_answer(answer),
        }
    }

    /// Encrypted bytes moved and the total, counting the chunk the command in flight moves.
    pub(super) fn progress(&self, in_flight: Option<Progress>) -> Option<(u64, u64)> {
        let past_block = |n: usize| n.saturating_sub(BLOCK_SIZE) as u64;
        match self {
            Self::Upload { machine, .. } => {
                let moving = in_flight.filter(|_| machine.is_putting());
                let now = moving.map_or(0, |p| past_block(p.sent));
                Some((machine.done() + now, machine.total()))
            }
            Self::Download(machine) => {
                let (done, total) = machine.progress();
                Some((
                    done + in_flight.map_or(0, |p| past_block(p.received)),
                    total,
                ))
            }
        }
    }
}
