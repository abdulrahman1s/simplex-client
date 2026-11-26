//! The official `simplex-chat` CLI driven over its WebSocket bot API (`simplex-chat -p <port>`):
//! commands go as `{"corrId","cmd"}`, answers and events come back as JSON text frames.
#![allow(
    clippy::disallowed_methods,
    reason = "the live tests wait on real time"
)]

use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

pub struct Cli {
    stream: TcpStream,
    reader: BufReader<TcpStream>,
    next: u64,
    /// Eviction: an event leaves when a wait takes it; a test reads them all in turn.
    events: VecDeque<Value>,
}

impl Cli {
    /// `url` is `ws://127.0.0.1:<port>`.
    pub fn connect(url: &str) -> Self {
        let addr = url.trim_start_matches("ws://").trim_end_matches('/');
        let mut stream = TcpStream::connect(addr).unwrap();
        write!(
            stream,
            "GET / HTTP/1.1\r\nHost: {addr}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\n\
             Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n"
        )
        .unwrap();
        let mut reader = BufReader::new(stream.try_clone().unwrap());
        let mut line = String::new();
        reader.read_line(&mut line).unwrap();
        assert!(line.contains(" 101 "), "no WebSocket upgrade: {line}");
        while line != "\r\n" {
            line.clear();
            reader.read_line(&mut line).unwrap();
        }
        Self {
            stream,
            reader,
            next: 0,
            events: VecDeque::new(),
        }
    }

    fn send_text(&mut self, text: &str) {
        let payload = text.as_bytes();
        let mut frame = vec![0x81];
        match payload.len() {
            n if n < 126 => frame.push(0x80 | u8::try_from(n).unwrap()),
            n if n < 65536 => {
                frame.push(0x80 | 126);
                frame.extend_from_slice(&u16::try_from(n).unwrap().to_be_bytes());
            }
            n => {
                frame.push(0x80 | 127);
                frame.extend_from_slice(&(n as u64).to_be_bytes());
            }
        }
        // A client masks; a fixed mask is enough for a test.
        let mask = [0x12, 0x34, 0x56, 0x78];
        frame.extend_from_slice(&mask);
        frame.extend(payload.iter().enumerate().map(|(i, b)| b ^ mask[i % 4]));
        self.stream.write_all(&frame).unwrap();
    }

    /// The next text message, or `None` after `left`.
    fn read_text(&mut self, left: Duration) -> Option<String> {
        self.stream
            .set_read_timeout(Some(left.max(Duration::from_millis(10))))
            .unwrap();
        let mut text = Vec::new();
        loop {
            let mut head = [0u8; 2];
            if self.reader.read_exact(&mut head).is_err() {
                return None;
            }
            self.stream.set_read_timeout(None).unwrap();
            let len = match head[1] & 0x7f {
                126 => {
                    let mut b = [0u8; 2];
                    self.reader.read_exact(&mut b).unwrap();
                    u64::from(u16::from_be_bytes(b))
                }
                127 => {
                    let mut b = [0u8; 8];
                    self.reader.read_exact(&mut b).unwrap();
                    u64::from_be_bytes(b)
                }
                n => u64::from(n),
            };
            let mut payload = vec![0u8; usize::try_from(len).unwrap()];
            self.reader.read_exact(&mut payload).unwrap();
            match head[0] & 0x0f {
                0x9 => continue,
                0x8 => return None,
                _ => text.extend(payload),
            }
            if head[0] & 0x80 != 0 {
                return Some(String::from_utf8(text).unwrap());
            }
        }
    }

    /// The command's answer (`resp`); events that come first are kept for `wait`.
    pub fn cmd(&mut self, cmd: &str) -> Value {
        self.next += 1;
        let corr = self.next.to_string();
        self.send_text(&json!({"corrId": corr, "cmd": cmd}).to_string());
        let end = Instant::now() + Duration::from_secs(60);
        loop {
            let left = end.saturating_duration_since(Instant::now());
            let text = self
                .read_text(left)
                .unwrap_or_else(|| panic!("no answer to {cmd}"));
            let v: Value = serde_json::from_str(&text).unwrap();
            if v["corrId"].as_str() == Some(corr.as_str()) {
                return v["resp"].clone();
            }
            self.events.push_back(v["resp"].clone());
        }
    }

    /// The first event `pick` takes, kept ones first; the ones it passes over stay kept.
    pub fn wait<T>(&mut self, what: &str, wait: Duration, pick: impl Fn(&Value) -> Option<T>) -> T {
        if let Some(i) = self.events.iter().position(|e| pick(e).is_some()) {
            let e = self.events.remove(i).unwrap();
            return pick(&e).unwrap();
        }
        let end = Instant::now() + wait;
        loop {
            let left = end.saturating_duration_since(Instant::now());
            let Some(text) = self.read_text(left) else {
                panic!("the CLI did not tell {what}");
            };
            let v: Value = serde_json::from_str(&text).unwrap();
            let resp = v["resp"].clone();
            if let Some(t) = pick(&resp) {
                return t;
            }
            self.events.push_back(resp);
        }
    }
}

/// Every chat item an event carries, as `(chatDir type, content)`.
pub fn items(resp: &Value) -> Vec<(String, Value)> {
    let list = resp["chatItems"].as_array().cloned().unwrap_or_default();
    list.iter()
        .map(|ci| {
            let it = &ci["chatItem"];
            (
                it["chatDir"]["type"].as_str().unwrap_or("").to_owned(),
                it["content"].clone(),
            )
        })
        .collect()
}
