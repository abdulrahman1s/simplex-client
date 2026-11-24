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
- CI for formatting, clippy, tests, wasm, MSRV 1.95, cargo-deny and docs.
- `Event::Offline(true)` at a start without a network, from the first relay check, and
  `Offline(false)` once a relay answers again.
- The preset relays in use are checked like own relays when the config names none.
- Chat notices for a peer's address switch and for ratchet re-syncs
  (`ChatNotice::AddressSwitch`, `ChatNotice::RatchetSync`).

### Fixed

- Connections agreed at agent version 7 move to version 8 once both sides speak it: every
  message offers the highest agent version (`AgentConfig::agent_versions`), and a higher one
  from the peer raises the stored version, as simplexmq's `updateConnVersion` does. A queue
  switch started the v7 way still ends the v7 way.
- A group member's chat version range follows each JSON message they send, not only the
  handshake (`updateMemberChatVRange`).
