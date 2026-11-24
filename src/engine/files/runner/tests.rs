//! The runner against the in-process XFTP server over direct pipes: whole files up and back, a cancel, a refused description.
use std::cell::RefCell;
use std::io::Write;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use crossbeam_channel::{Receiver, unbounded};

use super::FileRunner;
use crate::engine::files::queued::queue;
use crate::engine::files::transfers::{DownloadReq, FileDone, FileKey, Transfers, UploadReq};
use crate::engine::files::{FileStore, MemFiles};
use crate::engine::jobs::JobDone;
use crate::net::Socket;
use crate::smp::{Dialer, Pipe, PipeId, PipeInput, SmpError};
use crate::tls::testing::Pipe as Wire;
use crate::xftp::description::FileDescription;
use crate::xftp::tests::fake_server::Fake;

type Wires = Rc<RefCell<Vec<(PipeId, Wire, bool)>>>;

struct Direct {
    fake: Arc<Fake>,
    wires: Wires,
}

struct End(Wire);

impl Pipe for End {
    fn send_text(&mut self, _: &str) -> Result<(), SmpError> {
        Err(SmpError::Unexpected("text on a direct pipe"))
    }
    fn send_binary(&mut self, bytes: &[u8]) -> Result<(), SmpError> {
        self.0
            .write_all(bytes)
            .map_err(|e| SmpError::Io(e.to_string()))
    }
    fn close(&mut self) {
        let _ = self.0.shutdown(std::net::Shutdown::Both);
    }
}

impl Dialer for Direct {
    fn dial(&mut self, id: PipeId, _: &str, _: u16) -> Result<Box<dyn Pipe>, SmpError> {
        let (wire, _server) = self.fake.serve(0);
        let ours = wire.try_clone().unwrap();
        self.wires.borrow_mut().push((id, wire, false));
        Ok(Box::new(End(ours)))
    }
    fn tunnelled(&self) -> bool {
        false
    }
}

struct Rig {
    runner: FileRunner,
    wires: Wires,
    tx: crossbeam_channel::Sender<JobDone>,
    rx: Receiver<JobDone>,
    transfers: crate::engine::files::queued::QueuedTransfers,
    mem: MemFiles,
    now: u64,
    source: PathBuf,
}

impl Rig {
    fn new(fake: &Arc<Fake>, limit: u64) -> Self {
        let wires = Wires::default();
        let dialer = Direct {
            fake: Arc::clone(fake),
            wires: Rc::clone(&wires),
        };
        let (transfers, jobs) = queue();
        let mem = MemFiles::new(limit);
        let (tx, rx) = unbounded();
        let runner = FileRunner::new(jobs, Box::new(dialer), None, mem.clone());
        Self {
            runner,
            wires,
            tx,
            rx,
            transfers,
            mem,
            now: 1_000,
            source: PathBuf::new(),
        }
    }

    /// Moves bytes until a job reports its end; the progress seen on the way.
    fn until_done(&mut self) -> (FileDone, Vec<u8>) {
        let mut seen = Vec::new();
        for _ in 0..5_000 {
            self.now += 1;
            self.runner.tick(self.now);
            let ids: Vec<(PipeId, bool)> = self.wires.borrow().iter().map(|w| (w.0, w.2)).collect();
            for (id, opened) in ids {
                if !opened {
                    self.wires
                        .borrow_mut()
                        .iter_mut()
                        .for_each(|w| w.2 |= w.0 == id);
                    self.runner.on_input(id, PipeInput::Open, self.now);
                }
                self.runner.flush(self.now);
                let mut buf = vec![0u8; 65536];
                let read = (self.wires.borrow_mut().iter_mut().find(|w| w.0 == id))
                    .map(|w| w.1.read_within(&mut buf, Duration::from_millis(2)));
                if let Some(Ok(n)) = read.filter(|r| r.as_ref().is_ok_and(|n| *n > 0)) {
                    self.runner
                        .on_input(id, PipeInput::Binary(buf[..n].to_vec()), self.now);
                }
            }
            while let Ok(JobDone::File(done)) = self.rx.try_recv() {
                match done {
                    FileDone::Progress { percent, .. } => seen.push(percent),
                    other => return (other, seen),
                }
            }
        }
        panic!("the transfer never ended");
    }

    fn upload(
        &mut self,
        fake: &Fake,
        content: &[u8],
        cancel: Arc<AtomicBool>,
    ) -> (FileDone, Vec<u8>) {
        let source = self.mem.put("cat.png", content.to_vec(), false).unwrap();
        self.source = source.clone();
        let req = UploadReq {
            key: FileKey { friend: 1, item: 7 },
            members: None,
            job: 1,
            source,
            name: "cat.png".into(),
            work_dir: PathBuf::new(),
            servers: vec![fake.address.clone()],
            cancel,
        };
        self.transfers.upload(req, self.tx.clone());
        self.until_done()
    }

    fn download(&mut self, description: FileDescription) -> FileDone {
        let req = DownloadReq {
            key: FileKey { friend: 2, item: 9 },
            job: 2,
            description,
            work_dir: PathBuf::new(),
            dest_dir: PathBuf::new(),
            cancel: Arc::default(),
        };
        self.transfers.download(req, self.tx.clone());
        self.until_done().0
    }
}

#[test]
fn a_file_in_memory_goes_up_in_chunks_and_comes_back_through_the_runner() {
    let fake = Arc::new(Fake::new());
    let mut rig = Rig::new(&fake, 8 << 20);
    let content = crate::crypto::random_vec(1_500_000).unwrap();
    let (done, seen) = rig.upload(&fake, &content, Arc::default());
    let FileDone::Uploaded {
        result: Ok(sent), ..
    } = done
    else {
        panic!("{done:?}")
    };
    assert!(
        seen.windows(2).all(|w| w[0] < w[1]) && seen.last() == Some(&100),
        "{seen:?}"
    );
    assert!(
        rig.mem.take(&rig.source).is_none(),
        "the source left memory"
    );
    let fd = FileDescription::parse_valid(&sent.description).unwrap();
    assert!(fd.chunks.len() > 1);
    let FileDone::Downloaded {
        result: Ok(path), ..
    } = rig.download(fd)
    else {
        panic!()
    };
    assert!(rig.mem.exists(&path));
    assert_eq!(rig.mem.take(&path), Some(("cat.png".to_owned(), content)));
    assert!(rig.runner.next_deadline().is_none(), "nothing left to do");
    assert!(
        rig.wires.borrow().iter().all(|w| FileRunner::owns(w.0)),
        "never a relay's id"
    );
    rig.runner.close();
}

#[test]
fn a_cancelled_upload_ends_with_cancelled_and_frees_its_source() {
    let fake = Arc::new(Fake::new());
    let mut rig = Rig::new(&fake, 8 << 20);
    let (done, _) = rig.upload(&fake, b"x", Arc::new(AtomicBool::new(true)));
    let FileDone::Uploaded { result: Err(e), .. } = done else {
        panic!("{done:?}")
    };
    assert!(e.contains("cancelled"), "{e}");
    assert!(rig.mem.with_bytes(&rig.source, |_| ()).is_none());
}

#[test]
fn a_description_over_the_memory_limit_fails_without_a_connection() {
    let fake = Arc::new(Fake::new());
    let mut rig = Rig::new(&fake, 8 << 20);
    let (done, _) = rig.upload(&fake, &[5; 300_000], Arc::default());
    let FileDone::Uploaded {
        result: Ok(sent), ..
    } = done
    else {
        panic!()
    };
    let fd = FileDescription::parse_valid(&sent.description).unwrap();
    let dials = rig.wires.borrow().len();
    rig.mem.set_limit(100_000);
    let FileDone::Downloaded { result: Err(e), .. } = rig.download(fd) else {
        panic!()
    };
    assert!(e.contains("memory"), "{e}");
    assert_eq!(rig.wires.borrow().len(), dials);
}
