//! Whole in-memory transfers run over sessions to the in-process server: chunks, retries, extra recipients, a refused redirect.
use super::fake_server::Fake;
use super::rig::{Rig, T0};
use crate::crypto::{random_vec, sha512};
use crate::xftp::description::{FileDescription, Redirect, encode_description};
use crate::xftp::{
    Answer, Fetched, MemDownload, MemUpload, SessionEvent, Step, Uploaded, XftpError,
};

trait Machine {
    type Out;
    fn step(&mut self) -> Result<Step<Self::Out>, XftpError>;
    fn answer(&mut self, a: Answer) -> Result<(), XftpError>;
}

impl Machine for MemUpload {
    type Out = Uploaded;
    fn step(&mut self) -> Result<Step<Uploaded>, XftpError> {
        self.next_step()
    }
    fn answer(&mut self, a: Answer) -> Result<(), XftpError> {
        self.on_answer(a)
    }
}

impl Machine for MemDownload {
    type Out = Fetched;
    fn step(&mut self) -> Result<Step<Fetched>, XftpError> {
        self.next_step()
    }
    fn answer(&mut self, a: Answer) -> Result<(), XftpError> {
        self.on_answer(a)
    }
}

/// One session at a time to the one server, a fresh one after a temporary failure.
fn run<M: Machine>(fake: &Fake, m: &mut M) -> Result<M::Out, XftpError> {
    let mut rig: Option<Rig> = None;
    loop {
        let call = match m.step()? {
            Step::Done(out) => return Ok(out),
            Step::Call(call) => call,
        };
        let r = rig.get_or_insert_with(|| {
            let mut r = Rig::start(fake, 1 << 20);
            assert!(matches!(r.next(T0), SessionEvent::Ready));
            r
        });
        let s = &mut r.s;
        s.send(
            call.key.as_ref(),
            &call.entity,
            &call.cmd,
            &call.file,
            call.max_file,
            T0,
        )?;
        let answer = match r.next(T0) {
            SessionEvent::Answer(a) => a,
            other => panic!("{other:?}"),
        };
        if answer.as_ref().is_err_and(XftpError::is_temporary) {
            rig = None;
        }
        m.answer(answer)?;
    }
}

fn round_trip(fake: &Fake, content: &[u8], recipients: usize) -> (Uploaded, Fetched) {
    let servers = [fake.address.clone()];
    let mut up = MemUpload::new(content, "photo.jpg", &servers, recipients, None).unwrap();
    let total = up.total();
    let uploaded = run(fake, &mut up).unwrap();
    assert_eq!(up.done(), total);
    let fd = &uploaded.recipients[recipients - 1];
    let mut down = MemDownload::new(fd, 64 << 20).unwrap();
    let fetched = run(fake, &mut down).unwrap();
    (uploaded, fetched)
}

#[test]
fn a_multi_chunk_file_goes_up_and_comes_back_whole() {
    let fake = Fake::new();
    let content = random_vec(700_000).unwrap();
    let (up, got) = round_trip(&fake, &content, 4);
    assert!(up.recipients[0].chunks.len() > 1, "more than one chunk");
    assert_eq!(up.recipients.len(), 4);
    assert_eq!((got.file_name.as_str(), got.bytes), ("photo.jpg", content));
}

#[test]
fn a_temporary_server_error_is_tried_again_on_a_fresh_connection() {
    let fake = Fake::new();
    fake.store.lock().unwrap().fail_next = 2;
    let content = random_vec(10_000).unwrap();
    let (_, got) = round_trip(&fake, &content, 1);
    assert_eq!(got.bytes, content);
    let seen = fake.store.lock().unwrap().seen.clone();
    assert_eq!(
        &seen[..3],
        ["FNEW", "FNEW", "FNEW"],
        "the third try made the chunk"
    );
}

#[test]
fn recipients_past_two_hundred_are_added_with_fadd() {
    let fake = Fake::new();
    let (up, got) = round_trip(&fake, b"hello", 201);
    assert_eq!(got.bytes, b"hello");
    assert_eq!(up.recipients.len(), 201);
    assert!(fake.store.lock().unwrap().seen.contains(&"FADD".to_owned()));
}

/// Internal.hs `receiveViaCompleteFD` refuses redirects ("TODO re-enable redirects with relay
/// checks"): the inner description could name servers the user never approved.
#[test]
fn a_redirect_description_is_refused_before_any_server_is_asked() {
    let known = Fake::new();
    let unknown = Fake::new();
    let content = random_vec(30_000).unwrap();
    let mut up = MemUpload::new(
        &content,
        "big.bin",
        std::slice::from_ref(&unknown.address),
        1,
        None,
    )
    .unwrap();
    let inner = run(&unknown, &mut up).unwrap().recipients.remove(0);
    let text = encode_description(&inner);
    let mut up = MemUpload::new(
        text.as_bytes(),
        "",
        std::slice::from_ref(&known.address),
        1,
        None,
    )
    .unwrap();
    let mut outer: FileDescription = run(&known, &mut up).unwrap().recipients.remove(0);
    outer.redirect = Some(Redirect {
        size: inner.size,
        digest: inner.digest.clone(),
    });
    let seen = (
        known.store.lock().unwrap().seen.len(),
        unknown.store.lock().unwrap().seen.len(),
    );
    let e = MemDownload::new(&outer, 1 << 20).err().unwrap();
    assert!(
        matches!(&e, XftpError::Description(d) if d.contains("redirect")),
        "{e:?}"
    );
    assert_eq!(
        (
            known.store.lock().unwrap().seen.len(),
            unknown.store.lock().unwrap().seen.len()
        ),
        seen
    );
}

#[test]
fn a_description_over_the_memory_limit_is_refused_before_any_fetch() {
    let fake = Fake::new();
    let servers = [fake.address.clone()];
    let mut up = MemUpload::new(&[1; 100_000], "a", &servers, 1, None).unwrap();
    let fd = run(&fake, &mut up).unwrap().recipients.remove(0);
    let seen = fake.store.lock().unwrap().seen.len();
    assert!(matches!(
        MemDownload::new(&fd, 65_536),
        Err(XftpError::File(_))
    ));
    assert_eq!(fake.store.lock().unwrap().seen.len(), seen);
    assert_eq!(sha512(b"").len(), 64);
}
