//! XFTP transfers in memory over dialled pipes, for a client without threads: the browser's tunnel, or TCP on Node.js and Bun.

mod drive;
mod job;
mod link;
mod pool;
mod work;

pub use pool::FileRunner;

#[cfg(test)]
#[cfg(feature = "native")]
mod tests;
