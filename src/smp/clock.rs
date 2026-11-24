//! Monotonic time for timeouts behind a trait, and wall time for certificate dates, so tests can drive both (clippy.toml forbids `Instant::now` elsewhere).
#[cfg(not(all(target_arch = "wasm32", feature = "wasm")))]
use std::time::Instant;

pub trait Clock: Send + Sync {
    /// Milliseconds since an arbitrary fixed start.
    fn now_ms(&self) -> u64;
}

#[cfg(not(all(target_arch = "wasm32", feature = "wasm")))]
pub struct SystemClock(Instant);

/// `Instant::now` panics on wasm32-unknown-unknown, so the browser's wall clock stands in.
#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
pub struct SystemClock(f64);

#[cfg(not(all(target_arch = "wasm32", feature = "wasm")))]
impl SystemClock {
    #[must_use]
    #[allow(
        clippy::disallowed_methods,
        reason = "the one real monotonic clock of the SMP client"
    )]
    pub fn new() -> Self {
        Self(Instant::now())
    }
}

#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
impl SystemClock {
    #[must_use]
    pub fn new() -> Self {
        Self(js_sys::Date::now())
    }
}

impl Default for SystemClock {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(all(target_arch = "wasm32", feature = "wasm")))]
impl Clock for SystemClock {
    fn now_ms(&self) -> u64 {
        u64::try_from(self.0.elapsed().as_millis()).unwrap_or(u64::MAX)
    }
}

#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
impl Clock for SystemClock {
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "whole milliseconds, clamped at zero when the wall clock steps back"
    )]
    fn now_ms(&self) -> u64 {
        (js_sys::Date::now() - self.0).max(0.0) as u64
    }
}

/// Seconds since the Unix epoch: the default `ConnConfig::wall_clock`, which certificate
/// validity dates are checked against.
#[cfg(not(all(target_arch = "wasm32", feature = "wasm")))]
#[must_use]
#[allow(
    clippy::disallowed_methods,
    reason = "the one wall clock of the SMP and XFTP clients, for certificate dates"
)]
pub fn system_wall_clock() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

/// `SystemTime::now` panics on wasm32-unknown-unknown, so the browser's clock stands in.
#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
#[must_use]
#[allow(
    clippy::cast_possible_truncation,
    reason = "whole seconds since 1970 fit an i64 for millions of years"
)]
pub fn system_wall_clock() -> i64 {
    (js_sys::Date::now() / 1000.0) as i64
}
