# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `simplex_client::limits`: every limit and rule of the official SimpleX Chat 7.1.4 client in
  one module, each with its upstream source (reactions, file sizes, message size, mentions,
  names, bio, pictures, edit and delete windows, group roles), and `limits::check` for a
  request.
- The engine refuses what breaks them before sending, with new codes: `ReactionNotAllowed`,
  `FileTooLarge`, `TextTooLong`, `TooManyMentions`, `InvalidName`, `NotAllowed` and
  `PictureTooLarge` (`SX-CHT-018` to `024`); JS rejects with `reaction_not_allowed`,
  `file_too_large`, `text_too_long`, `too_many_mentions`, `invalid_name`, `not_allowed` and
  `picture_too_large`. `Client::request` fails at once for the rules that need no lookup.
- Display and group names are checked as upstream's `mkValidName` does, at most 50
  characters; a changed bio is at most 160 bytes; a new store's name is checked at start.
- A friend's fourth reaction on a message, or one on a message without content, is ignored.

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

- JS rejections carry a snake-case `code` everywhere (`no_connection` instead of
  `SX-CHT-004`); a file over the memory limit rejects with `file_too_large`, not `too_large`.
- JS calls that queue a request (`send`, `react`, `edit`, `delete`, …) now reject when the
  request breaks a limit, instead of resolving.
- `setFileLimit` never goes above 1 GiB, the official client's file limit.
- A message whose JSON is over 15 602 bytes is refused even when it would compress to fit,
  as upstream refuses it.
- `types::REACTION_EMOJI` is `limits::REACTIONS`, a slice; `chat::reaction::REACTION_EMOJI` is
  gone.
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

- A repeated `x.grp.inv` with the same link updates the group only while the user has not
  joined yet, and only from the friend who invited. 7.1.4 rewrites the user's member id, role
  and the group profile even after joining.
- At most 50 joins through a group's link wait for their connection; past that the oldest is
  dropped with its connection. 7.1.4 accepts group link requests without limit, each with a
  member, a connection and server queues.
- `x.grp.link.acpt` acts only on a member waiting for approval or review. 7.1.4 takes any
  member, so a moderator could demote an owner or bring a removed member back; such a message
  is now ignored.
- A moderator's "block for all" (`x.grp.mem.restrict`) is applied, as `xGrpMemRestrict` does:
  the member is kept as `blocked_by_admin`, shown as `blocked` in the member list, their
  messages are collapsed and never unread, their mentions are dropped, their pictures are not
  fetched by themselves and their items stay out of the history a host sends. A
  `ChatNotice::MemberBlocked` item tells it, and an unblock undoes it. The user's own unblock
  does not.
- A friend's deletion of its own message is taken only if the relay took it within 78 hours
  of the message (`limits::RCV_DELETE_WINDOW_MS`, Subscriber.hs `rcvItemDeletable`); a later
  one is ignored. `AgentEvent::Msg` carries the relay's time (`broker_ts`) for it.
- A message that skips, repeats or breaks the chain before it (`MsgIntegrity`) now shows a
  `ChatNotice::Integrity` item before it, as simplex-chat's `CIRcvIntegrityError` does: "skipped
  message ID", "unexpected message ID", "incorrect message hash", "duplicate message ID". A
  receipt whose hash differs shows the message as `Delivery::DeliveredBadHash`; in a group, one
  such receipt wins over the good ones. Engine records are format 2 now; format 1 still reads.
- A forward's time with a year past 9999, or a date out of range, no longer overflows (a
  panic in debug builds, a wrapped time in release): the JSON `x.grp.msg.forward` is refused,
  as Aeson refuses it, and a binary forward takes the time it arrived.
- A received file description that is a redirect is refused, on native and in memory, as
  simplex-chat 7.1.4 does ("redirect not allowed"): its inner description could send the
  download to servers the user never approved. The file shows as failed. This client never
  sends redirects in chat, as 7.1.4 does not.
- `DownloadParams::max_size` (breaking): a native download fetches no more encrypted bytes than
  the offer allows (`min(size, 5 GiB) + 10 MiB`), so a file offered as small cannot fill the
  disk. `MemDownload::progress` now returns the pair itself.
- A file description nested more than eight levels deep, or longer than its replica limit
  allows, is refused before parsing recurses. A line of dashes in a description used to
  overflow the stack and abort the process, with no click needed for pictures.
- Connections agreed at agent version 7 move to version 8 once both sides speak it: every
  message offers the highest agent version (`AgentConfig::agent_versions`), and a higher one
  from the peer raises the stored version, as simplexmq's `updateConnVersion` does. A queue
  switch started the v7 way still ends the v7 way.
- A group member's chat version range follows each JSON message they send, not only the
  handshake (`updateMemberChatVRange`).
