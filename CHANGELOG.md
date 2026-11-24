# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- File transfers in the wasm build (browser, Node.js and Bun): `friend.sendFile`,
  `group.sendFile`, `message.download()` and the `fileProgress` event, over a sans-I/O XFTP
  session on a sans-I/O HTTP/2 client.
- Private message routing (SMP proxying, `PRXY`/`PFWD`) with the chat's defaults; set it with
  `ClientBuilder::private_routing` / `allow_direct`, or `privateRouting` / `allowDirect` in JS.
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
- The same build on Node.js 22+ and Bun, over direct TCP with no tunnel; a `simplex-client` npm
  package (built by `scripts/build-js.sh`, not published) and `examples/node/bot.mjs`.
- `store`: an in-memory backend beside SQLite, and `export_snapshot`/`open_snapshot` for a
  sealed, versioned snapshot of either.
- CI for formatting, clippy, tests, wasm, MSRV 1.95, cargo-deny and docs.
- Files in the wasm build, in browsers, Node.js and Bun: `friend.sendFile(bytes, name)`,
  `group.sendFile(bytes, name)`, `message.download()` resolving with `{name, bytes}`, and a
  `fileProgress` event. Transfers run in memory, up to `setFileLimit` bytes (64 MiB unless
  set), over the tunnel or TCP.
- `h2::H2Client` and `xftp::XftpSession`: sans-I/O HTTP/2 and XFTP, driven by bytes in and out.
- `Event::Offline(true)` at a start without a network, from the first relay check, and
  `Offline(false)` once a relay answers again.
- Private message routing: messages, confirmations, SKEY, LKEY and LGET for someone else's
  relay go through one of our own relays (`PRXY`/`PFWD`), so that relay never sees our IP
  address. `Config::proxy_mode` and `Config::proxy_fallback` choose when; they default to the
  chat's `unknown` (proxy unless the relay is one of ours) and `protected` (never fall back to
  a direct connection, as this client has no SOCKS or onion transport). `AgentConfig` has the
  same settings, defaulting to the agent's `never`.
- The preset relays in use are checked like own relays when the config names none.
- Chat notices for a peer's address switch and for ratchet re-syncs
  (`ChatNotice::AddressSwitch`, `ChatNotice::RatchetSync`).
- The JS package runs on Bun too; CI runs the JS tests on Node.js and Bun.
- Objects to act on: `Message` (`reply`, `react`, `edit`, `delete`, `author`, `chat`),
  `FriendRequest` (`accept`, `reject`), `Friend` (`send`, `remove`, `history`, `show_code`),
  `Group` (`send`, `add_member`, `leave`, `members`), `GroupInvite` and `Member`, in Rust and
  in JS. The client keeps the chat list: `friend(id)`, `friends()`, `groups()`, and
  `create_group` returns the new `Group` (JS: `client.friends`, `client.groups`,
  `createGroup`).

### Changed

- **Breaking:** `simplex_client::Event` is now the high-level event: `Ready`, `Message`,
  `MessageUpdate`, `Request`, `FriendAdd`, `FriendRemove`, `GroupAdd`, `GroupRemove`,
  `GroupInvite`, `MemberAdd`, `Offline` and `Raw` for the rest. The engine's events stay at
  `simplex_client::types::Event`. `simplex_client::Friend` and `FriendRequest` are now the
  objects; the chat list's records are `types::model::Friend` and `FriendRequest`. A
  wasm-only build has no root `Event`; use `types::Event`.
- **Breaking (JS):** events carry objects and are named `ready`, `message`, `messageUpdate`,
  `request`, `friendAdd`, `friendRemove`, `groupAdd`, `groupRemove`, `groupInvite`,
  `memberAdd`, `offline`, `online`, `change` and `raw` (was `event`); `chats` is gone. The
  client's id-taking methods (`send`, `reply`, `accept`, `members`, …) moved onto the
  objects, every method returns a Promise, `profile()` is the `user` getter, and `once` is new.

### Fixed

- Connections agreed at agent version 7 move to version 8 once both sides speak it: every
  message offers the highest agent version (`AgentConfig::agent_versions`), and a higher one
  from the peer raises the stored version, as simplexmq's `updateConnVersion` does. A queue
  switch started the v7 way still ends the v7 way.
- A group member's chat version range follows each JSON message they send, not only the
  handshake (`updateMemberChatVRange`).
- A blocking TLS read that fails for any reason but a timeout now fails every later read and
  write too, as the sans-I/O client already did; a retry could otherwise skip a forged record.
- TLS refuses handshake messages that span a key change (RFC 8446 section 5.1): the encrypted
  server flight sent unencrypted in the ServerHello record, anything after the server Finished
  in its record, and anything after a KeyUpdate in its record.
- Stricter TLS 1.3 checks: the ServerHello must carry `supported_versions` = TLS 1.3, an
  extension type may appear once, EncryptedExtensions may only answer what the ClientHello
  offered, a CertificateRequest must carry `signature_algorithms`, records over the plaintext
  limit are refused with `record_overflow`, and a record sequence number never wraps.
- The blocking SMP connect (TCP, TLS and the SMP handshake) ends with `SmpError::Timeout` at
  `ConnConfig::connect_timeout`, then the next host is tried; the timeout used to bound each
  socket read, so a server dripping a byte now and then held a connect forever. Blocking XFTP
  connects bound TCP and TLS by the connect timeout and the handshake by one per request.
