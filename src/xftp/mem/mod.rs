//! Whole transfers in memory as state machines: each asks for one command on one server and takes its answer, so any driver can run them (Agent.hs send and receive workers, without files or threads).

mod download;
mod download_finish;
mod step;
mod upload;
mod upload_chunk;
mod upload_stage;

pub use download::{Fetched, MemDownload};
pub use step::{Answer, Call, Step};
pub use upload::MemUpload;
