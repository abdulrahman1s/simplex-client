# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Initial extraction from the author's app, ported from simplex-chat v7.1.4 and simplexmq
  commit `fdbcb24d`.
- Protocol stack: SMP queues, the agent's double ratchet with the post-quantum KEM, chat
  messages, contacts, groups, XFTP files and calls.
- Low-level public modules: `smp`, `agent`, `ratchet`, `chat`, `crypto`, `xftp`, `tls`, `h2`,
  `encoding`, `store`, `engine`, `types`.
- High-level `Client` with a builder, an event stream and request methods (`native` feature,
  on by default).
- `wasm` feature: the pure protocol core and small wasm-bindgen bindings for
  `wasm32-unknown-unknown`.
- The full client on `wasm32-unknown-unknown`: a JS `Client` (`Client.start`, named events,
  Promise-returning calls) on a single-threaded runtime, reaching relays through a WebSocket
  byte tunnel with TLS inside wasm; `examples/browser` is a minimal chat page.
- The same build on Node.js 22+, over direct TCP with no tunnel; a `simplex-client` npm
  package (built by `scripts/build-js.sh`, not published) and `examples/node/bot.mjs`.
- `store`: an in-memory backend beside SQLite, and `export_snapshot`/`open_snapshot` for a
  sealed, versioned snapshot of either.
- CI for formatting, clippy, tests, wasm, MSRV 1.95, cargo-deny and docs.
