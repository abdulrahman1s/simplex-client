# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project aims to follow
[Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Application data. `Request::SendData { chat, tag, json, fallback_text }`
  (`client.send_data`; JS `friend.sendData(tag, data, fallbackText)`, `group.sendData`)
  sends `json` as `x.msg.new` with the content type `tag`. The official apps do not know the
  type and show `fallback_text`, as simplex-chat 7.1.4 does for any `MCUnknown`; a live test
  checks that the official CLI 7.1.4 keeps it as an item with that text. Content types
  registered at start (`ClientBuilder::data_tag`, `Config::data_tags`; JS
  `Client.start({ dataTags })`) arrive as `Event::Data { chat, member, tag, json }` (JS
  `data`) instead of a message. Neither side keeps an item. A tag the apps know or with
  spaces, and data with a `type` or `text` key, are refused. Data under a tag not
  registered still arrives as a message with its fallback text. **Breaking:** `Request`,
  `types::Event` and `client::Event` have new variants and `Config` a new field.
- `Friend.pq` (JS `friend.pq`): whether the connection's ratchet uses the post-quantum KEM
  both ways, as negotiated, not as asked for. It is what the ratchet used for the last
  message sent and the last received (`rcSndKEM`, `rcRcvKEM`), checked after each one as
  simplex-chat 7.1.4 keeps `pqSndEnabled` and `pqRcvEnabled` (`connPQEnabled`), so it is
  false until a message has gone each way and stays false with a peer without PQ. The chat
  list goes out again when it changes. A live test checks it against the official CLI
  7.1.4. **Breaking:** `Friend` has a new field and the `AgentPort` trait a new method,
  `pq_on`.
- `ClientBuilder::start` takes an exclusive OS file lock (`std::fs::File::try_lock` on
  `chat.db.lock` in the client's folder), held by the engine thread until `close()` returns.
  A second client on the same folder, in this process or another, fails with the new
  `Error::Locked(folder)` instead of writing the same store. **Breaking:** `client::Error`
  has a new variant.
- `ClientBuilder::key_zeroizing(Zeroizing<String>)` takes the store passphrase in a buffer
  that wipes itself, and `Config::key` is now a `Zeroizing<String>`. The client keeps no
  other copy: the engine thread takes the key out of its config and wipes it as soon as the
  store is open (or the open failed), and the wasm client's options wipe theirs once the
  store is open. `ClientBuilder::key(impl Into<String>)` still works; an owned `String` is
  taken and wiped the same way. `Config`'s `Debug` shows the key as `<redacted>`. Not wiped:
  a `&str` the caller passed, the JS string in wasm, and the hashing state Argon2id
  (and HKDF, for a store from before Argon2id) keeps while it runs. **Breaking:**
  `Config::key` is a `Zeroizing<String>`.
- Sends are tied to their outcome. `Client::send`, `reply`, `send_file`, `send_link`,
  `send_live`, `send_data` and `forward` (and the same methods on `Friend`, `Group`, `Chat`
  and `Message`) return a `SendHandle`, and what becomes of the send arrives as
  `client::Event::SendStatus { handle, member, status }`: `Queued { item }` with the
  message it made, or `Refused { code, detail }` at once instead of an unlinked
  `Raw(Failed)`; then `Sent` or `Failed { detail }` for the friend, or for each `member` of
  a group, as the agent reports `SENT` or `MERR`. Underneath, `Request::Tagged { tag,
  request }` wraps any request, and `types::Event::SendStatus { tag, member, status }` (JS
  `sendStatus`) tells its outcome. A file's message goes after its upload, so only its
  `Queued` or `Refused` is told; the JS methods do not tag their sends yet. **Breaking:**
  those methods return `Result<SendHandle>` instead of `Result<()>` (a `send(…)?` used as a
  `match` arm's value needs a `;`), and `Request` and both `Event`s have new variants.

### Fixed

- Removing a friend (and a group member, and leaving a group) deleted the connection with
  everything still waiting to be sent on it, so the peer never got `x.direct.del` and a
  message sent just before was sometimes lost. The agent's `delete` now does what
  `deleteConnectionsAsync` with `waitDelivery` does in simplexmq: the receive queue goes at
  once, so nothing more arrives, and the connection stays until what waits on it is
  delivered or given up, across a restart too, or 2 days at most
  (`connDeleteDeliveryTimeout`). A live test removes a friend right after writing to them
  and sees the official CLI 7.1.4 get both the message and the deletion.

## [0.2.0] - 2026-11-25

### Added

- `client.saveChanges()` (JS) and `Store::export_changes` save only the rows written since the
  last save, and `Client.start({snapshot, changes})` and `Store::open_snapshot_with_changes`
  open a snapshot with the changes kept after it. A change costs what it wrote: with 30,000
  messages in wasm, `save()` takes 95 ms for 22.5 MB, and `saveChanges()` after one message
  under 1 ms for 1 KB. Each delta is MAC'd and chained to its snapshot and the delta before
  it; a missing, repeated or reordered one is `StoreError::ChangesDoNotFollow` (JS
  `changes_do_not_follow`), and deltas of an older snapshot are skipped. **Breaking:**
  `StoreError` has this new variant, so an exhaustive `match` on it needs an arm. The README and the
  Node bot example keep a log of changes instead of saving the whole store on every change.
- Disappearing messages, as SimpleX Chat 7.1.4 has them. `Request::SetTimer { chat, ttl }`
  (`friend.set_timer`, `group.set_timer`; JS `friend.setTimer(seconds)`,
  `group.setTimer(seconds)`, `null` for off) offers a friend a timer in the profile they are
  sent, or sets a group's as an owner. Each message then carries its `ttl` and is deleted with
  its file: the user's own counts from sending, a received one from when `MarkRead` reads it.
  A friend's timer is taken over from their profile without an answer, as the official apps
  do, and the conversation gets a `ChatNotice::Timer` whenever the timer that applies changes.
  The timers last across restarts: what came due while the client was off goes at the next
  start. `Friend.timer` and `ChatGroup.timer` give the timer that applies, and
  `ChatMessage.timed` (JS `message.ttl`, `message.expiresAt`) a message's own. **Breaking:**
  `Request`, `RequestKind` and `ChatNotice` have new variants and `ChatMessage`, `Friend` and
  `ChatGroup` new fields.

- Chat preferences, the user's own and per friend, as SimpleX Chat 7.1.4 has them.
  `Request::SetPrefs { friend, prefs }` (`client.set_prefs`, `friend.set_prefs`; JS
  `client.setPrefs({ voice: "no" })`, `friend.setPrefs(...)`) sets full delete, reactions,
  voice, files, calls and the "allow" of disappearing messages to `always`, `yes` or `no`,
  for every friend or over the user's own for one. Each friend whose profile it changes is
  sent the new one (`APIUpdateProfile`, `APISetContactPrefs`). What is on between the user and
  a friend follows both sides (`prefEnabled`; only disappearing messages are symmetric), and
  shows in `Friend.features` (JS `friend.features`); `Friend.prefs` and `ChatProfile.prefs` hold
  what the user set. A voice message from a friend the user does not allow is no longer kept.
  **Breaking:** `Request` and `RequestKind` have new variants, `Friend` and `ChatProfile` new
  fields, and `UserRecord` is no longer `Eq`.

- Group admin, as SimpleX Chat 7.1.4 has it: `Request::SetMemberRole`, `BlockMemberForAll`,
  `RemoveMember` (with or without the member's messages), `DeleteGroup`, `SetGroupProfile`
  (name and description), `SetGroupPrefs` and `Moderate`, with the senders' role checks from
  Commands.hs (`APIMembersRole`, `APIBlockMembersForAll`, `APIRemoveMembers`, `APIDeleteChat`,
  `APIUpdateGroupProfile`, `APIDeleteMemberChatItem`). Rust: `member.set_role`,
  `block_for_all`, `remove`, `group.delete`, `set_profile`, `set_prefs`, `message.moderate`;
  JS the same in camelCase. A role change goes to every current member and the member too; a
  block to the others; a removal to all before the member's connection goes once delivered; an
  invited member's new role goes in their invitation again. Deleting a group removes
  everything kept about it. `GroupMember.role`, `blocked_for_all`, `ChatGroup.role` and
  `description` show the result, and a role change made by anyone now leaves a
  `ChatNotice::MemberRole`. A live test runs all of it against two official CLIs.
  **Breaking:** `Request`, `RequestKind` and `ChatNotice` have new variants, `GroupMember` and
  `ChatGroup` new fields.

- What the user keeps for themselves, as SimpleX Chat 7.1.4 has it: `Request::DeleteForMe`
  (`APIDeleteChatItem` internal), `ClearChat` (`APIClearChat`), `MarkReadUpTo`
  (`APIChatItemsRead`), `SetUnreadMark` (`APIChatUnread`), `SetAlias` (`APISetContactAlias`,
  `APISetGroupAlias`) and `SetMessageTtl` (`APISetChatItemTTL`, `APISetChatTTL`), which
  deletes messages older than a TTL every half hour, for every chat or one, keeping a group's
  last 12 hours. Rust: `message.delete_for_me`, `mark_read_up_to`, `friend.clear`,
  `set_unread_mark`, `set_alias`, `set_message_ttl` (and on `Group`), `client.set_message_ttl`;
  JS the same in camelCase. `Friend.alias`, `item_ttl`, `ChatGroup.alias`, `item_ttl`,
  `ChatPreview.marked_unread` and `ChatProfile.item_ttl` show them, and `Event::Cleared` says a
  chat was cleared. **Breaking:** `Request`, `RequestKind` and `Event` have new variants, and
  `Friend`, `ChatGroup`, `ChatPreview` and `ChatProfile` new fields.

- Voice messages: `FileKind::Voice` (JS `"voice"`) sends `MCVoice` with its length, refused
  where the friend or the group does not allow the user's voice messages (Commands.hs
  `featureAllowed SCFVoice forUser`, Internal.hs `prohibitedGroupContent`). Received ones show
  as `MessageBody::Voice(Media)` with their `duration` instead of a plain file. JS `sendFile`
  takes `{ caption, duration, preview }` as a fourth argument, which it could not send before.
  **Breaking:** `MessageBody::Voice` holds a `Media` instead of `secs` and `wave`, and
  `FileKind` has a new variant.

- Link previews: `Request::SendLink { chat, text, preview, reply_to }` (`client.send_link`;
  JS `friend.sendLink(text, { uri, title, description, image })`) sends `MCLink`, and a
  received link's preview is kept in `ChatMessage.link` (JS `message.link`) instead of being
  dropped; group history keeps it too. **Breaking:** `Request` has a new variant and
  `ChatMessage` a new field.

- Forwarding: `Request::Forward { from, item, to }` (`friend.forward(&message)`,
  `group.forward`; JS the same) sends a message to another chat as the user's own with
  `forward: true` (`APIForwardChatItems`, `mcForward`): text and link previews as they are, a
  file again when it is here, else its caption. A received forward shows in
  `ChatMessage.forwarded` (JS `message.forwarded`). **Breaking:** `Request` has a new variant,
  `ChatMessage` a new field, and the engine's `send_file` takes a `FileMeta`.

- Live messages, written as the other side watches: `Request::SendLive` and `UpdateLive { done }`
  (`friend.send_live`, `message.update_live`; JS `sendLive`, `updateLive`) send `live: true`
  until done (`APISendMessages` live, `APIUpdateChatItem`). Received ones show in
  `ChatMessage.live`, and their updates are not marked edited (`edited = itemLive /= Just
  True`). A disappearing live message counts from when it is done. An edit now carries the
  message's timer, as upstream's does. **Breaking:** `Request` has new variants and
  `ChatMessage` a new field.

- Direct chats with group members: `Request::ContactMember { group, member, text }`
  (`member.contact`; JS the same) makes a one-time invitation and sends it with `text` over the
  member's group connection in `x.grp.direct.inv` (`APICreateMemberContact`,
  `APISendMemberContactInvitation`), where the group allows the user direct messages. The member
  shows as a friend, connecting, until they accept. **Breaking:** `Request` has a new variant.

- "Fix connection": `Request::SyncRatchet { chat, member, force }` (`friend.fix_connection`,
  `member.fix_connection`; JS `fixConnection`) starts a ratchet re-sync with a friend or a group
  member, with `force` even while the connection works (`APISyncContactRatchet`,
  `APISyncGroupMemberRatchet`). The engine already started one by itself when a connection
  needed it. The agent port's `synchronize_ratchet` takes `force`. **Breaking:** `Request` has a
  new variant and `AgentPort::synchronize_ratchet` a new argument.

- Address settings: `Request::SetAddressSettings { auto_accept, auto_reply }`
  (`client.set_address_settings`; JS `setAddressSettings({ autoAccept, autoReply })`) accepts
  requests to the address as they come and sends the auto-reply to each friend made through it
  once connected, however the request was accepted (`APISetAddressSettings`, Subscriber.hs
  `autoAccept`, `sendAutoReply`).
  `Request::ShareAddress(on)` (JS `shareAddress`) shows the address in the profile every
  friend gets (`APISetProfileAddress`), and deleting the address takes it out. A friend's shown
  address is in `Friend.address`; the settings in `ChatProfile`. **Breaking:** `Request` has
  new variants, `Friend` and `ChatProfile` new fields, and `UserRecord` and `AddressRecord` new
  fields.

- Incognito: `Request::CreateIncognitoInvite`, `ConnectIncognito(link)` and
  `AcceptIncognito(request)` (`client.create_incognito_invite`, `connect_incognito`,
  `request.accept_incognito`; JS `createInvite({ incognito: true })`,
  `connect(link, { incognito: true })`, `request.acceptIncognito()`) make a friend who knows
  the user only by a random name from upstream's word lists (`generateRandomProfile`), with
  the default preferences and no picture, in the invitation's link data, the handshake and
  every later profile. `Friend.incognito` holds the name. A retried invitation stays incognito.
  Joining a group through an incognito friend, inviting one into a group, and connecting to a
  group link incognito are refused, as none of them could keep the user's profile out
  (`CEContactIncognitoCantInvite`); an incognito friend is judged against the default
  preferences, as they were sent (`mergeUserChatPrefs'`). **Breaking:** `Request` has new variants and
  `Friend` a new field; JS `createInvite` and `connect` take an optional options argument.

### Fixed

- The profile sent to friends said disappearing messages were allowed, so an official app
  offered its user a timer, but this client kept their messages for good. The timer is now
  kept. A profile change now goes to each friend with their own timer in it, so renaming no
  longer turns a friend's timer off.

### Changed

- Through the tunnel, the TLS ClientHello goes out right behind the tunnel hello instead of
  waiting for `{"ok":true}`, so connecting to a relay or file server takes one round trip
  less. The relay's bytes are still refused until the tunnel says yes.
- File uploads in wasm keep up to 768 KiB unacknowledged instead of 256 KiB. The pinned XFTP
  server grants about 1 MiB, so an upload moves three times as much per round trip.
- Tunnel Worker: it looks the relay's name up while the per-IP counter decides, rather than
  after. A refused tunnel still reads `rate_limited` or `too_many_tunnels`, and now costs a
  DNS lookup too.
- The JS packages are built with a `wasm-release` profile (fat LTO, one codegen unit) and
  `wasm-opt -O3`: the module shrinks from 5.1 MB to 3.3 MB (1.5 MB to 1.3 MB gzipped),
  Ed25519 signing and verifying get about 10% faster, and Ed448 key generation about 25%.
  `scripts/build-js.sh` skips `wasm-opt` with a warning when binaryen is missing, except
  under `REQUIRE_WASM_OPT`, which CI and the release set.
- The SQLite store goes through rusqlite instead of ormlite (sqlx) on a private tokio runtime.
  A sealed write takes about 20 µs instead of 80 µs and a read about 6 µs instead of 26 µs.
  tokio, sqlx and rustls leave the native build: 156 crates in the lock file instead of 275,
  and the stripped echo bot shrinks from 8.9 MB to 7.8 MB. Store files and their settings are
  unchanged (WAL, `synchronous = NORMAL`, `secure_delete`, `foreign_keys`, a 5 s busy timeout).
- A whole snapshot of a memory store is encoded from its rows in place instead of from a copy
  of them: at 50,000 rows (30 MB) natively it takes 36 ms instead of 66 ms.

## [0.1.0] - 2026-11-24

### Security

- The HPACK integer decoder refuses a sixth continuation byte before shifting, so a 32-bit
  (wasm) build no longer shifts past its word size on a hostile header block.
- The HTTP/2 frame reader drops taken bytes once per read instead of once per frame, so a
  server that sends many tiny frames costs linear time, not quadratic.
- Tunnel Worker: a lease renewal that reaches the per-IP counter after the tunnel's release no
  longer takes the slot back for up to 30 minutes.
- A snapshot whose item or id counter is at its maximum is refused as damaged when the counter
  would move, instead of panicking (debug) or wrapping to a negative id (release).
- **Breaking:** the store key is required: `ClientBuilder::start` fails without `.key(…)`
  (it used to default to an empty key that anyone could open), and JS `Client.start` refuses
  an empty `key`. A new store derives its key with Argon2id (19 MiB, 2 passes, 1 lane) over
  the passphrase and a random per-store salt kept in its `kdf` meta row, instead of one
  unsalted HKDF. Stores made before still open with their old key and are not upgraded.
- Snapshots are authenticated as a whole: format 2 ends with an HMAC over every byte, so a
  changed, deleted or rolled-back row or a rewound counter is refused on open. The id counters
  are checked against a sealed copy in both backends, which a store from before gets on its
  first open, and SQLite overwrites deleted rows (`secure_delete`). In SQLite, putting back an
  older sealed row is still not detected.
- **Breaking (tunnel Worker):** it is no longer an open TCP proxy by default. Without
  `ALLOWED_HOSTS` it reaches only the preset SimpleX SMP and XFTP relays; `ALLOWED_HOSTS` adds
  hosts and `ALLOWED_HOSTS=*` allows any public host. Every record the client sends before the
  relay's first encrypted record is checked (a second ClientHello after a HelloRetryRequest
  too), the relay must answer in TLS records, and the per-IP limits fail closed when the
  counter or the rate limiter errors.
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
- Server certificates must be valid now: `verify_server_chain` checks every certificate's
  notBefore and notAfter (no skew allowance, as upstream), in SMP connects, PKEY and XFTP. The
  time comes from the new `ConnConfig::wall_clock` and `xftp::ClientConfig::wall_clock`
  (default `smp::system_wall_clock`); `verify_server_chain` and `HandshakeConfig` take it too.
- SMP and XFTP connects check the server's pinned identity inside the TLS handshake, before the
  client Finished, as upstream does; new `TlsStream::connect_pinned`, `TlsClient::start_pinned`,
  `tls::Pin` and `TlsError::Identity`.
- The tunnel pool redials at once after a `1000 lifetime` close only when the session was up
  and the tunnel lived an hour; earlier ones wait out the back-off, so a hostile tunnel cannot
  force a reconnect loop.
- A forwarded command's answer (PRES) under the command's own nonce is refused as the proxy's
  error before it is opened; it could only be the proxy reflecting the client's own PFWD body.
- A relay can no longer replace a connection's per-queue key through a message header to force
  a ratchet re-sync: once set, the stored key is always used, as simplexmq does.
- A message that fails to decrypt no longer advances or saves the ratchet, so a tampered copy
  can't destroy the key the genuine message needs.
- Ed25519 signatures are checked with `verify_strict`.
- Ratchet and agent secrets are wiped from memory after encoding and loading.
- Envelopes above the supported client versions are refused, and invitation short links get
  the same server check as addresses.

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

- `Ratchet::decrypt` no longer advances on failure; `MsgEncryptKey::encrypt_msg` takes `self`.
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
  does not. `x.grp.mem.intro` carries the block both ways (`memberRestrictions`), so a member
  who joins later learns of it.
- A friend's deletion of its own message is taken only if the relay took it within 78 hours
  of the message (`limits::RCV_DELETE_WINDOW_MS`, Subscriber.hs `rcvItemDeletable`); a later
  one is ignored. `AgentEvent::Msg` carries the relay's time (`broker_ts`) for it.
- A message that skips, repeats or breaks the chain before it (`MsgIntegrity`) now shows a
  `ChatNotice::Integrity` item before it, as simplex-chat's `CIRcvIntegrityError` does: "skipped
  message ID", "unexpected message ID", "incorrect message hash", "duplicate message ID". A
  receipt whose hash differs shows the message as `Delivery::DeliveredBadHash`; in a group, one
  such receipt wins over the good ones. A message that does not decrypt shows a
  `ChatNotice::DecryptionError` (header, too many skipped, earlier message, other), and the
  same error again counts on that notice, as `CIRcvDecryptionError` does. A blocked friend's
  chat gets neither. Engine records are format 2 now; format 1 still reads.
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

[Unreleased]: https://github.com/abdulrahman1s/simplex-client/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/abdulrahman1s/simplex-client/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/abdulrahman1s/simplex-client/releases/tag/v0.1.0
