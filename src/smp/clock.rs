//! Monotonic time for timeouts, behind a trait so tests can drive it (clippy.toml forbids `Instant::now` elsewhere).
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
