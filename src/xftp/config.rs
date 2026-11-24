//! How long an XFTP connection waits for each step (Client.hs `XFTPClientConfig`, `xftpClientTimeout`).
use std::time::Duration;

#[derive(Debug, Clone, Copy)]
pub struct ClientConfig {
    pub connect_timeout: Duration,
    /// How long one command may take besides moving its bytes.
    pub command_timeout: Duration,
    /// Extra time per KiB of chunk, so a slow link can finish a 4 MiB chunk.
    pub per_kib: Duration,
    /// How often a waiting read wakes up to look at the cancel flag.
    pub poll: Duration,
    /// How long one write may stall; a timed-out write ends the connection, so this is generous.
    pub write_timeout: Duration,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(20),
            command_timeout: Duration::from_secs(30),
            per_kib: Duration::from_millis(25),
            poll: Duration::from_millis(250),
            write_timeout: Duration::from_secs(30),
        }
    }
}

impl ClientConfig {
    /// Milliseconds one command may take when it moves `bytes` of chunk either way.
    #[must_use]
    pub fn command_budget_ms(&self, bytes: usize) -> u64 {
        let kib = u32::try_from(bytes / 1024).unwrap_or(u32::MAX);
        u64::try_from((self.command_timeout + self.per_kib * kib).as_millis()).unwrap_or(u64::MAX)
    }
}
