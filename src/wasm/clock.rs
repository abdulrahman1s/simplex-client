//! Wall time from `Date.now()`, for the engine and the agent: `SystemTime::now` panics on wasm32.
use crate::agent::client::WallClock;
use crate::engine::Clock;

#[allow(
    clippy::cast_possible_truncation,
    reason = "whole milliseconds since 1970 fit an i64 for millions of years"
)]
fn now() -> i64 {
    js_sys::Date::now() as i64
}

#[derive(Debug, Default, Clone, Copy)]
pub struct JsClock;

impl Clock for JsClock {
    fn wall_ms(&self) -> i64 {
        now()
    }
}

impl WallClock for JsClock {
    fn now_ms(&self) -> i64 {
        now()
    }
}
