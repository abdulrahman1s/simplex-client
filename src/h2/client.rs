//! A sans-I/O HTTP/2 client connection with one request in flight (RFC 9113): bytes in through `feed`, bytes out through `take_outgoing`.
use super::error::H2Error;
use super::frames::{
    CLIENT_PREFACE, DEFAULT_MAX_FRAME, DEFAULT_WINDOW, SETTINGS_ENABLE_PUSH,
    SETTINGS_HEADER_TABLE_SIZE, SETTINGS_INITIAL_WINDOW_SIZE, SETTINGS_MAX_FRAME_SIZE,
    encode_goaway, encode_settings, encode_window_update,
};
use super::hpack_decode::HpackDecoder;
use super::reader::FrameReader;
use super::response::{Progress, Request, Response};
use super::state::{Exchange, HeaderBlock, Options};

pub struct H2Client {
    pub(super) reader: FrameReader,
    /// Frames to send; drained by `take_outgoing`, so at most the control frames of one `feed`,
    /// one header block and one body batch.
    pub(super) out: Vec<u8>,
    pub(super) hpack: HpackDecoder,
    pub(super) opts: Options,
    pub(super) next_stream: u32,
    pub(super) peer_max_frame: usize,
    pub(super) peer_initial_window: i64,
    pub(super) conn_send: i64,
    pub(super) conn_recv_consumed: u32,
    pub(super) block: Option<HeaderBlock>,
    pub(super) going_away: bool,
    pub(super) ex: Option<Exchange>,
    pub(super) done: Option<Response>,
    pub(super) progress: Progress,
    pub(super) dead: bool,
}

impl H2Client {
    /// Queues the client preface and SETTINGS; the server's SETTINGS arrive with the first response.
    #[must_use]
    pub fn open(opts: Options) -> Self {
        let settings = [
            (SETTINGS_HEADER_TABLE_SIZE, 4096),
            (SETTINGS_ENABLE_PUSH, 0),
            (SETTINGS_INITIAL_WINDOW_SIZE, opts.stream_window),
            (SETTINGS_MAX_FRAME_SIZE, DEFAULT_MAX_FRAME as u32),
        ];
        let mut out = [CLIENT_PREFACE, &encode_settings(&settings)].concat();
        let extra = i64::from(opts.connection_window) - DEFAULT_WINDOW;
        if extra > 0 {
            out.extend(encode_window_update(0, extra as u32));
        }
        Self {
            reader: FrameReader::default(),
            out,
            hpack: HpackDecoder::new(4096, opts.max_header_bytes),
            opts,
            next_stream: 1,
            peer_max_frame: DEFAULT_MAX_FRAME,
            peer_initial_window: DEFAULT_WINDOW,
            conn_send: DEFAULT_WINDOW,
            conn_recv_consumed: 0,
            block: None,
            going_away: false,
            ex: None,
            done: None,
            progress: Progress::default(),
            dead: false,
        }
    }

    #[must_use]
    pub fn is_usable(&self) -> bool {
        !self.dead && !self.going_away
    }

    /// A request is in flight or its response has not been taken.
    #[must_use]
    pub fn is_busy(&self) -> bool {
        self.ex.is_some() || self.done.is_some()
    }

    /// Starts `req`, keeping a copy of its body to send as the windows allow; the response may
    /// carry at most `max_body` bytes of body.
    pub fn start(&mut self, req: &Request<'_>, max_body: usize) -> Result<(), H2Error> {
        if !self.is_usable() {
            return Err(H2Error::Closed);
        }
        if self.is_busy() {
            return Err(H2Error::Protocol("a request is already in flight"));
        }
        if self.next_stream > 0x7fff_fffd {
            self.dead = true;
            return Err(H2Error::Protocol("stream ids exhausted"));
        }
        let ex = Exchange::new(self.next_stream, self, req.body.to_vec(), max_body);
        self.next_stream += 2;
        self.progress = Progress::default();
        self.send_headers(&ex, req);
        self.ex = Some(ex);
        Ok(())
    }

    /// The next bytes to send: control frames, then up to one batch of the body the windows allow.
    pub fn take_outgoing(&mut self) -> Option<Vec<u8>> {
        if let Some(mut ex) = self.ex.take() {
            self.send_batch(&mut ex);
            self.progress = ex.progress();
            self.ex = Some(ex);
        }
        if self.out.is_empty() {
            return None;
        }
        Some(std::mem::take(&mut self.out))
    }

    /// Body bytes moved by the current or last request.
    #[must_use]
    pub fn progress(&self) -> Progress {
        self.progress
    }

    /// The response of the request in flight, once it is complete.
    pub fn take_response(&mut self) -> Option<Response> {
        self.done.take()
    }

    /// Gives up on the connection: the request in flight is dropped and nothing more is accepted.
    pub fn abandon(&mut self) {
        self.dead = true;
        self.ex = None;
    }

    /// Graceful close: GOAWAY, after anything still waiting in `take_outgoing`.
    pub fn close(&mut self) {
        if !self.dead {
            self.out.extend(encode_goaway(0, 0));
        }
        self.abandon();
    }
}
