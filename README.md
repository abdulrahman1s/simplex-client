# simplex-client

A SimpleX Chat protocol client in Rust: connect, chat, run groups and send files through a
small, plain API.

[![CI](https://github.com/abdulrahman1s/simplex-client/actions/workflows/ci.yml/badge.svg)](https://github.com/abdulrahman1s/simplex-client/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](LICENSE)

> **Unofficial and unaudited.** This project is not affiliated with or endorsed by SimpleX
> Chat Ltd. It has not had a security review. Do not rely on it where a flaw would hurt someone.

## Features

- SMP queues over TLS, with the agent's double ratchet and the post-quantum KEM
- Private message routing: messages reach other people's relays through one of yours, so
  their relay never sees your IP address
- Chat messages, contacts, groups, replies, edits, deletes and reactions, link previews,
  forwarding, live messages
- Disappearing messages, for a friend or a group
- Application data for bots and apps, which the official apps show as a fallback text
- Channels, as owner and as subscriber, through official SimpleX chat relays: create one on
  the relays you name, post as the channel (signed or not), join from a channel link, receive
  the relays' history replay, leave. **Not end-to-end encrypted**; see Security notes
- Chat preferences (full delete, reactions, voice, files, calls), your own and per friend
- Group admin: roles, blocking for everyone, removing members, moderation, the group's name,
  description and preferences, and deleting a group
- Local housekeeping: delete for me, clear a chat, read up to a message, unread marks,
  aliases, and deleting old messages after a TTL
- XFTP file upload and download, with file crypto and descriptions, natively and in the
  browser, Node.js and Bun
- Calls (signalling and frame crypto)
- SQLite store sealed with a passphrase
- A threaded engine and a high-level `Client`; no async runtime
- A `wasm32-unknown-unknown` build of the whole client for browsers, Node.js and Bun, reaching
  relays through a WebSocket tunnel or direct TCP
- Low-level modules stay public if you need to go below `Client`

### Private message routing

Messages to a relay that isn't one of yours go through one of your relays (SMP proxying), so
the recipient's relay sees that relay's address, never yours. That is the chat's default; you
can make it stricter:

```rust
use simplex_client::{Client, ProxyMode};

let client = Client::builder("./data")
    .key("store passphrase")
    .private_routing(ProxyMode::Always) // Always, Unknown (default), Never
    .allow_direct(false)                // never fall back to sending directly (default)
    .start()?;
```

```js
const client = await Client.start({
  key: "passphrase",
  privateRouting: "always", // "always" | "unknown" (default) | "never"
  allowDirect: false,
});
```

## Why this library

The official way to embed SimpleX Chat is its Haskell core: link `libsimplex` with its GHC
runtime, or run the `simplex-chat` CLI as a separate process and drive it over its WebSocket
API, which is what the official TypeScript client does. This crate speaks the same protocols
natively instead.

| | Official core | simplex-client |
|---|---|---|
| Native, x86_64 Linux | `libsimplex` + GHC runtime: **238 MB** · `simplex-chat` CLI: **87 MB** | a whole echo bot, SQLite included: **7.8 MB** (stripped release) |
| Browser | no | **3.3 MB** wasm, **1.3 MB** gzipped |
| Node.js and Bun | a CLI process beside yours, over a local WebSocket | in-process wasm, direct TCP |
| Language | Haskell, with a C FFI or a JSON-over-WebSocket API | Rust crate or a JS class |
| Runtime | GHC runtime and its threads | plain threads natively, one event loop in wasm; no async runtime |
| Memory safety | — | `#![forbid(unsafe_code)]` |

What that buys you:

- **One dependency, one process.** No second binary to ship, start, supervise and talk to,
  and no JSON protocol between your code and the client.
- **It runs where the official core can't:** browsers and edge runtimes, through
  wasm32. On Node.js and Bun it opens relay connections itself.
- **Small enough to bundle.** A bot, a desktop app or a web page carries the client in a few
  megabytes, and a sandboxed process needs no Haskell runtime.
- **A small API.** Start a `Client`, read its events, call `send`, `reply` and `accept`.
  The protocol layers below stay public when you need them.
- **Byte-compatible.** It is ported from simplex-chat v7.1.4 and simplexmq `fdbcb24d`,
  checked against golden bytes from the reference and live against the official CLI and
  servers.

What the official core still has that this crate lacks:

- **Maturity and audits.** The official core is the reference implementation and has had
  security audits; this crate has not.
- **Tor and SOCKS.** There is no onion or SOCKS transport, so private routing is the only IP
  protection.
- **Coverage of newer features.** Features added after 7.1.4 are not tracked.

Sizes were measured on 2026-11-24: the official 7.1.4 CLI release asset, and `libsimplex`
7.0.3 with its shared libraries as packaged on Linux.

## Quick start

The crate is not published on crates.io. Depend on it through git:

```toml
[dependencies]
simplex-client = { git = "https://github.com/abdulrahman1s/simplex-client", tag = "v0.6.0" }
```

An echo bot:

```rust
use simplex_client::{Client, Event};

fn main() -> simplex_client::Result<()> {
    let client = Client::builder("./bot-data")
        .name("Echo Bot")
        .key("store passphrase")
        .start()?;
    println!("my address: {}", client.create_address()?);
    for event in client.events() {
        match event {
            Event::Request(request) => request.accept()?,
            Event::FriendAdd(friend) => {
                friend.send("Hi! I echo what you write.")?;
            }
            Event::Message(msg) if !msg.mine() && !msg.text().is_empty() => {
                msg.reply(msg.text())?;
            }
            _ => {}
        }
    }
    Ok(())
}
```

Events carry objects that act on their own: `Message` (`reply`, `react`, `edit`, `delete`,
`author()`, `chat()`), `FriendRequest` (`accept`, `reject`), `Friend` (`send`, `remove`,
`history`, `show_code`) and `Group` (`send`, `add_member`, `leave`, `members`). The client
keeps the chat list: `client.friend(id)`, `client.friends()`, `client.groups()`, and
`client.create_group(name)` returns the new `Group`. `request(Request)` is the escape hatch
for anything else.

Each send (`send`, `reply`, `send_file`, `send_link`, `send_live`, `send_data`, `forward`)
returns a `SendHandle`, and what becomes of it arrives as `Event::SendStatus { handle,
member, status }`: `Queued { item }` with the message it made, or `Refused { code, detail }`
at once; then `Sent` or `Failed { detail }` for the friend, or for each `member` of a group.
A file is `Sent` once the last part of its description went after the upload, and `Failed`
with no member when the upload fails or the file is cancelled or deleted first.

### Events

| Event | Fires when |
|---|---|
| `Ready(profile)` | the engine has started |
| `Message(message)` | a message arrives, or your own is sent |
| `MessageUpdate(message)` | a message is edited, deleted, reacted to, delivered, or its file moves |
| `Request(request)` | someone asks to connect through your address |
| `FriendAdd(friend)` / `FriendRemove(friend)` | a friend finishes connecting / is removed |
| `GroupAdd(group)` / `GroupRemove(group)` | you make or join a group / leave it |
| `GroupInvite(invite)` | you are invited into a group |
| `MemberAdd(member)` | a member connects with you: someone new, or each one there when you join |
| `Offline(bool)` | no relay is reachable, or one is again |
| `Data { chat, member, tag, json }` | application data arrives under a tag you registered |
| `SendStatus { handle, member, status }` | a send was queued or refused, then sent or failed |
| `FriendUpdate(friend)` / `GroupUpdate(group)` | a friend or group changed (with `roster_deltas`) |
| `Raw(event)` | anything else the engine reports, and every chat list |

A client with many friends can start with `.roster_deltas(true)` (JS `rosterDeltas: true`):
the engine then sends only the friends and groups that changed, which arrive as
`FriendUpdate` and `GroupUpdate` beside the adds and removes, and a whole list only when
`client.refresh()` asks for it.

The engine's own events are `simplex_client::types::Event` (`cargo doc --open`).

Friends and groups from before a restart are not announced again: `FriendAdd` and `GroupAdd`
are for what is new. The client's list is complete as soon as `start` returns, so read it
there, then follow the events:

```rust
let client = Client::builder("./bot-data").key(key).start()?;
let mut known: HashMap<i64, Friend> = client.friends().into_iter().map(|f| (f.id(), f)).collect();
for event in client.events() {
    match event {
        Event::FriendAdd(f) => { known.insert(f.id(), f); }
        Event::FriendRemove(f) => { known.remove(&f.id()); }
        _ => {}
    }
}
```

## WebAssembly

The `wasm` feature builds the whole client for `wasm32-unknown-unknown`: the engine, the
agent and an in-memory store, driven by one `setTimeout` loop, plus the codec bindings
(`src/wasm/`). A browser has no raw TCP, so relays are reached through a WebSocket tunnel, a
Cloudflare Worker that only pipes bytes. TLS runs inside wasm, pinned by the relay's
fingerprint, so the tunnel sees ciphertext only. Files (XFTP) stay native-only.

```sh
cargo build --lib --profile wasm-release --target wasm32-unknown-unknown --no-default-features --features wasm
wasm-bindgen --target web --out-dir pkg target/wasm32-unknown-unknown/wasm-release/simplex_client.wasm
wasm-opt -O3 pkg/simplex_client_bg.wasm -o pkg/simplex_client_bg.wasm   # optional, binaryen
```

The `wasm-release` profile adds fat LTO and one codegen unit to `release`. With `wasm-opt` the
module is a third smaller than a plain release build (5.1 MB before).

`getrandom` 0.3 needs its JS backend on wasm. This repository sets it in `.cargo/config.toml`;
projects that depend on the crate must set it too:

```sh
RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo build --target wasm32-unknown-unknown ...
```

```js
import init, { Client } from "simplex-client";

await init();

const client = await Client.start({
  name: "Alice",
  key: "passphrase",
  tunnel: "wss://simplex-tunnel.citizen6775.workers.dev/v1",
});

client.on("request", (request) => request.accept());

client.on("message", async (message) => {
  if (message.mine) return;
  await message.reply(`You said: ${message.text}`);
});

console.log("My address:", await client.createAddress());
```

`client.save()` returns the whole sealed store; pass it back as `snapshot` to pick up where you
left off. `client.saveChanges()` returns only what changed since the last save, so its cost
follows the change, not the size of the store; keep each one in order and pass them back as
`changes` with the snapshot they follow. A missing or reordered one makes `Client.start` reject
with `changes_do_not_follow`. JS events are named in camelCase: `ready`, `message`, `messageUpdate`, `request`,
`friendAdd`, `friendRemove`, `groupAdd`, `groupRemove`, `groupInvite`, `memberAdd`, `offline`,
`online`, `change` (save now) and `raw` (every engine event). `client.friends` and
`client.groups` are `Map`s of `Friend` and `Group`, and `await client.createGroup(name)` gives
a `Group`.

[`examples/browser`](examples/browser) is a minimal chat page with the full JS API.

### Node.js and Bun

The same build runs on Node.js 22+ and Bun. There no tunnel is needed: the client dials
relays over direct TCP (`node:net`), and TLS still runs inside wasm. Each
[release](https://github.com/abdulrahman1s/simplex-client/releases) carries a ready-built
package, so no Rust toolchain is needed:

```sh
npm install https://github.com/abdulrahman1s/simplex-client/releases/download/v0.6.0/simplex-client-0.6.0.tgz
bun add https://github.com/abdulrahman1s/simplex-client/releases/download/v0.6.0/simplex-client-0.6.0.tgz
```

Installing from git (`npm install github:abdulrahman1s/simplex-client`) builds it instead,
which needs Rust with the `wasm32-unknown-unknown` target and `wasm-bindgen-cli` 0.2.130. It
runs binaryen's `wasm-opt` too when it is installed.

```js
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { Client } from "simplex-client";

const SNAPSHOT = "bot.snapshot";
const CHANGES = "bot.changes"; // one file per saveChanges(), numbered in order
const changes = () =>
  readdirSync(CHANGES).sort((a, b) => a - b).map((f) => readFileSync(`${CHANGES}/${f}`));

const saved = existsSync(SNAPSHOT);
mkdirSync(CHANGES, { recursive: true });
const bot = await Client.start({
  name: "Echo Bot",
  key: "passphrase",
  snapshot: saved ? readFileSync(SNAPSHOT) : undefined,
  changes: saved ? changes() : [],
});
// A whole save starts a new chain; changes kept before it are skipped from now on.
writeFileSync(SNAPSHOT, bot.save());
rmSync(CHANGES, { recursive: true });
mkdirSync(CHANGES);
let next = 0;

bot.on("request", (request) => request.accept());
bot.on("friendAdd", (friend) => friend.send("Hi! I echo what you write."));
bot.on("message", async (message) => {
  if (message.mine || !message.text) return;
  await message.reply(message.text);
});

bot.on("change", () => writeFileSync(`${CHANGES}/${next++}`, bot.saveChanges()));
console.log("My address:", await bot.createAddress());
```

`transport: "tcp"` picks direct TCP explicitly; a `tunnel` URL uses the WebSocket tunnel
instead. `scripts/build-js.sh` builds both packages (`pkg/web` and `pkg/node`), and
[`examples/node/bot.mjs`](examples/node/bot.mjs) is the bot above with a safer save (each file
written whole, a whole save every 1,000 changes, a new chain after a failed write); run it
with `node examples/node/bot.mjs` or `bun examples/node/bot.mjs`.

### Files

Files go over XFTP, natively and in wasm, in the browser through the same tunnel. In wasm
they are held in memory, so files over 64 MiB are refused (`client.setFileLimit(bytes)`
changes that, up to the 1 GiB every client keeps to).

```js
const photo = await fs.promises.readFile("cat.jpg"); // or new Uint8Array(await file.arrayBuffer())
await friend.sendFile(photo, "cat.jpg", "image", { caption: "Mine", preview: smallJpeg });
await friend.sendFile(recording, "note.m4a", "voice", { duration: 12 }); // seconds

client.on("message", async (message) => {
  if (!message.file) return; // { name, size, kind } or null; kind "voice" for a voice message
  const { name, bytes } = await message.download();
  console.log(`got ${name}: ${bytes.length} bytes`);
});

client.on("fileProgress", ({ percent, sending }) => {
  console.log(`${sending ? "sending" : "receiving"}: ${percent}%`);
});
```

### Disappearing messages

A timer makes new messages in a chat go after that many seconds: the user's own from when they
are sent, received ones from when the chat is marked read. Each one goes with its file, and
timers last across restarts.

```js
await friend.setTimer(86400); // a day; the friend is offered it in your profile
await friend.setTimer(null);  // off, for both of you
await group.setTimer(3600);   // the group's timer: owners only

client.on("message", (message) => {
  if (message.ttl) console.log(`lasts ${message.ttl}s, goes at ${message.expiresAt}`);
});
```

A friend's timer comes with their profile and becomes yours for them, as in the official
apps. `friend.timer` and `group.timer` say which timer applies. A received message counts down
only once it is read, so a bot that never calls `markRead()` keeps them; call it after
handling a message if they should go.

### Chat preferences

Each friend gets your preferences in your profile, and the features on between you follow
both sides' (Types/Preferences.hs `prefEnabled`). Set your own, or override them for one
friend; each feature is `"always"`, `"yes"` or `"no"`, and one left out falls back to your own,
then to the default.

```js
await client.setPrefs({ voice: "no", full_delete: "yes" }); // to each friend it changes
await friend.setPrefs({ voice: "always" });                // just for this friend
console.log(friend.features.voice); // { for_user: false, for_contact: true }
```

`"always"` keeps a feature on for your friend's messages even when they said no to it for
yours. A friend's reaction, voice message, call or full delete that you don't allow is
refused, as the official apps refuse it.

### Group admin

Admins and owners manage a group the way the official apps do, with the same role checks
(`assertUserGroupRole`): changing a role needs admin and at least both roles, blocking for
everyone needs moderator, removing needs admin, and the group's profile and preferences need
the owner.

```js
const [member] = (await group.members()).filter((m) => m.name === "bob");
await member.setRole("moderator");  // "observer" | "author" | "member" | "moderator" | "admin" | "owner"
await member.blockForAll(true);     // hides their messages for every member
await member.remove(true);          // true: what they wrote shows deleted for everyone
await member.contact("hi!");        // a direct chat, where the group allows direct messages
await message.moderate();           // a member's message, deleted for everyone
await group.setProfile("crew", "Welcome!");
await group.setPrefs({ voice: { on: false }, files: { on: true, role: "moderator" } });
await group.delete();               // an owner deletes it for every member too
```

### Incognito

A friend made incognito knows you only by a random name (an adjective and a noun, from the
official apps' lists), with the default preferences and no picture; your profile changes never
reach them.

```js
const link = await client.createInvite({ incognito: true });
await client.connect(theirLink, { incognito: true });
client.on("request", (r) => r.acceptIncognito());
console.log(friend.raw.incognito); // the name they know you by
```

Groups always get your own profile; joining one incognito is not supported yet.

### Your address

```js
await client.setAddressSettings({ autoAccept: true, autoReply: "Hi! I'm a bot." });
await client.shareAddress(true); // shows it in the profile every friend gets
console.log(friend.raw.address); // a friend's shared address, if they show one
```

### Link previews

The page makes a link's preview (the client fetches nothing) and sends it with the text:

```js
await friend.sendLink("see https://simplex.chat", {
  uri: "https://simplex.chat", title: "SimpleX", description: "Private messaging",
  image: smallJpeg, // optional
});
client.on("message", (m) => m.link && console.log(m.link.title));

await carol.forward(message); // as yours, marked forwarded; message.forwarded on arrival

// A live message: your friend sees it as you type, then you finish it.
await friend.sendLive("Hel");
client.once("message").then((m) => m.updateLive("Hello!", true)); // m.live until done
```

A forwarded file goes again only when it is here (sent from here, or downloaded and still in
memory in wasm); otherwise its caption goes as text, as the official apps do.

### Application data

Programs can talk to each other in the same chats people use. Data goes as a message
content type the official apps do not know; they show its fallback text instead, as SimpleX
Chat 7.1.4 does for any type it does not know (`MCUnknown`). Nothing is kept on either side
for data under a tag the receiver registered:

```rust
let client = Client::builder("bot").key(key).data_tag("app.poll").start()?;
let mut poll = serde_json::Map::new();
poll.insert("q".into(), "lunch?".into());
client.send_data(friend.chat(), "app.poll", poll, "A poll: lunch?")?;
// Event::Data { chat, member, tag, json } for "app.poll" from others
```

```js
const client = await Client.start({ ...options, dataTags: ["app.poll"] });
await friend.sendData("app.poll", { q: "lunch?" }, "A poll: lunch?");
client.on("data", ({ chat, member, tag, data }) => chat.sendData(tag, { a: 1 }, "Voted"));
```

A tag is not one of the types the apps know (`text`, `link`, `image`, …) and has no spaces;
the data has no `type` or `text` key, since those hold the tag and the fallback. Data must fit
in one message, as text does. It is not kept, so a message the relay delivers again after a
restart, before it was acknowledged, comes again, and a group member's data that reaches you
both from them and through the host comes twice. A blocked member's data is dropped. Data
under a tag the client did not register arrives as a message with its fallback text.

### Your own housekeeping

These stay on your device and nobody is told:

```js
await message.deleteForMe();        // with its file
await message.markReadUpTo();       // later messages stay unread
await friend.clear();               // every message, with their files
await friend.setAlias("Mum");       // your own name for them
await group.setUnreadMark(true);
await client.setMessageTtl(7 * 86400); // delete messages older than a week, every chat
await friend.setMessageTtl(0);         // except this one; null follows your own again
```

Old messages are looked for every half hour, as the official apps do, and a group's messages
of the last 12 hours stay whatever time they say they were sent.

## Limits

The client keeps the limits the official SimpleX Chat 7.1.4 client checks before it sends,
so it refuses what that client would refuse. `simplex_client::limits` has them all, each with
its upstream source. The main ones:

| Limit | Value | Upstream |
|---|---|---|
| Reactions | 👍 👎 😀 😂 😢 ❤️ 🚀 ✅ (`REACTIONS`) | Protocol.hs `mrEmojiChar` |
| Reactions per person per message | 3 (`MAX_REACTIONS`) | Internal.hs `maxMsgReactions` |
| File size | 1 GiB (`MAX_FILE_SIZE`) | Description.hs `maxFileSize` |
| Message as JSON | 15 602 bytes (`MAX_ENCODED_MSG_LENGTH`) | Protocol.hs `maxEncodedMsgLength` |
| Mentions per message | 3 (`MAX_SENT_MENTIONS`) | Internal.hs `maxSndMentions` |
| Display and group names | 50 characters, no leading punctuation, none of `` .,;/\#@'"`~ `` (`is_valid_name`) | Commands.hs `mkValidName` |
| Bio | 160 bytes as JSON (`MAX_BIO_BYTES`) | apps `MAX_BIO_LENGTH_BYTES` |
| Profile and group picture | a 12 500-character `data:` URI (`MAX_PICTURE_URI`) | Commands.hs `maxProfileImageSize` |
| Edit your message | for 1 day (`EDIT_WINDOW_MS`) | Messages.hs `deletable'` |
| Delete your message for everyone | for 30 hours (`DELETE_WINDOW_MS`) | Commands.hs `APIDeleteChatItem` |
| Send in a group | `author` or above (`ROLE_TO_SEND`) | Commands.hs `assertUserGroupRole` |
| Add members, make a group link | `admin` or above | Commands.hs `APIAddMember`, `APICreateGroupLink` |
| Change the group picture | `owner` | Commands.hs `runUpdateGroupProfile` |

The chat's preferences apply as well: reactions, files or full deletion turned off by a
friend or a group are refused. Received messages are held to the same rules, so a reaction
outside the set or a fourth one from the same person is ignored. Some values are listed for
reference only, since the official apps use them as targets rather than refusals or this
client has no request they apply to: the inline SMP file size, the image and preview sizes,
voice length, auto-receive sizes and the roles to remove, moderate or block for everyone. The
group preference on SimpleX links is not checked yet.

A request that breaks a limit fails with its own code before anything is sent:

| Rust `ErrorCode` | Code | JS `code` | When |
|---|---|---|---|
| `ReactionNotAllowed` | `SX-CHT-018` | `reaction_not_allowed` | not one of the reactions, given twice, or a fourth one |
| `FileTooLarge` | `SX-CHT-019` | `file_too_large` | a file over 1 GiB, or over the JS memory limit |
| `TextTooLong` | `SX-CHT-020` | `text_too_long` | a message, caption or bio too long |
| `TooManyMentions` | `SX-CHT-021` | `too_many_mentions` | more than 3 mentions |
| `InvalidName` | `SX-CHT-022` | `invalid_name` | a name the official client would change |
| `NotAllowed` | `SX-CHT-023` | `not_allowed` | a role, preference or time window does not allow it |
| `PictureTooLarge` | `SX-CHT-024` | `picture_too_large` | a picture over the profile limit |

In Rust, `Client::request` and the methods on it return `Error::Failed { code, .. }` at once
for what needs no lookup (the reaction set, text size, mention count, names, bio); the rest
arrives as `Event::Failed`. In JS every call rejects with `{ code, detail }`: the limit codes
above, `request_failed`, `no_connection`, `bad_link`, `call_failed` and `stream_failed` from
the engine, and `closed`, `timeout`, `bad_argument`, `bad_option`, `wrong_key`, `file_gone`,
`file_failed` and `file_cancelled` from the client itself. Calls that only queue a request
(`send`, `react`, `edit`, `delete`, …) resolve once the engine has taken it and reject only
for a limit; other refusals arrive as a `raw` event. A send (`send`, `reply`, `sendLink`,
`sendLive`, `sendData`, `forward`) resolves with its tag instead, and what becomes of it,
its refusal too, arrives as `sendStatus` events with that tag: `queued` with the message it
made, then `sent` or `failed` for the friend or each group member. `sendFile` resolves with
its message, whose id the `queued` status names, and is `sent` once its description went
after the upload.

## Browser tunnel (Cloudflare Worker)

Browsers cannot open raw TCP. [`worker/`](worker/README.md) is a small Cloudflare Worker that
only pipes WebSocket frames to a relay's TCP port and back. It is the transport the browser
build uses.

**A free public tunnel is running**, so you can start without deploying anything:

```
wss://simplex-tunnel.citizen6775.workers.dev/v1
```

It is offered as is, with no uptime promise, and with the per-IP limits below. For anything
real, deploy your own (see below): it takes three commands and fits Cloudflare's free plan.

- **The Worker sees only TLS ciphertext.** The client runs TLS 1.3 in wasm and pins the relay
  by the fingerprint in its `smp://` address. SMP queue encryption and the end-to-end double
  ratchet sit on top of that.
- **Cloudflare can see** your IP, the relay host and port, and the timing and sizes of the
  fixed 16 KiB SMP blocks.
- **Cloudflare cannot see** message content, keys, contacts or which queues you use.
- **It is not an open proxy.** By default it reaches only the preset SimpleX relays (SMP and
  XFTP), on ports 5223 and 443. Every record the client sends before the relay's first
  encrypted one must be a TLS ClientHello with ALPN `smp/1` or `xftp/1` and no SNI. Every IP
  is rate-limited and has a cap on open tunnels, and when the limits cannot be checked the
  tunnel is refused. Your own relays need a Worker of your own with them in `ALLOWED_HOSTS`
  (`*` allows any public host).

Deploy your own and pass its URL as `tunnel`, with one click:

[![Deploy to Cloudflare](https://deploy.workers.cloudflare.com/button)](https://deploy.workers.cloudflare.com/?url=https://github.com/abdulrahman1s/simplex-client/tree/main/worker)

or from the command line:

```sh
cd worker && npm ci
npx wrangler login
npx wrangler deploy
```

The protocol, limits and settings are in [worker/README.md](worker/README.md).

### Channels

A channel is a broadcast group: its owners post as the channel, its subscribers join from the
channel's link as `observer` and may not post. Posts go from the owner to each chat relay and
from each relay to every subscriber; relays replay the last 100 posts (those with text or a
file) to whoever joins. A chat relay is the official `simplex-chat` CLI run with `--relay`.

```rust
use simplex_client::ChatRelay;
owner.set_chat_relays(vec![ChatRelay { address: relay_address, name: "my relay".into() }])?;
let news = owner.create_channel("news")?;          // invites, then publishes, each relay
// news.info().channel: the link, and each relay's status (Event::Raw(ChannelRelay) on change)
owner.send_data(news.chat(), "app.post", post, "A post")?;        // as the channel
owner.send_signed_data(news.chat(), "app.post", post2, "A post")?; // signed by the owner

owner.test_chat_relay(&relay_address)?;            // `/relay test`: RelayTestResult
owner.add_channel_relays(news.id(), vec![another])?; // `APIAddGroupRelays`

subscriber.connect(&channel_link)?;                // reads the link, joins every relay
// Event::Data { member: None, .. } for registered tags, Message with Author::Channel otherwise
```

```js
await owner.setChatRelays([{ address: relayAddress, name: "my relay" }]);
const news = await owner.createChannel("news");
await news.sendSignedData("app.post", { a: 1 }, "A post");
const tested = await owner.testChatRelay(relayAddress); // { address, name, failure }
await owner.addChannelRelays(news.id, [{ address: other, name: "other" }]);
```

- **Your relays only.** `set_chat_relays` names the relays new channels are made on, and only
  those are ever used. `ChatRelay::presets()` lists simplex-chat 7.1.4's preset chat relays,
  run by SimpleX Chat Ltd, but nothing uses them unless you pass them yourself: unlike the
  preset SMP relays they are no default. SimpleX Chat's preset-server policy limits
  third-party apps to "user-to-user messaging only or … chat bots sending messages requested
  by users"; a channel through their relays may fall outside it.
- **Data-only posts are not replayed.** A relay replays a post to later subscribers only if it
  has text or a file (7.1.4 Internal.hs `processContentItem`). `send_data` puts its
  `fallback_text` in the post's text, so a channel post with an empty `fallback_text` reaches
  who is subscribed when it is sent and nobody who joins later. 7.1.4 sends such posts too, so
  the crate does not refuse them.
- **Relay tests.** `test_chat_relay` does what 7.1.4's `/relay test` does: it reads the
  address's link, joins it with a random challenge (PQ off), and passes only if the relay
  answers with the challenge signed by the address's own key within 40 seconds. The test
  connection is deleted either way; a failure names its step (`RelayTestStep`).
- **More relays.** `add_channel_relays` invites more relays to a channel the user owns, through
  the same invitation, acceptance and roster steps as creation, then publishes the link again
  naming every relay that may serve. Subscribers who join later join all of them; while the
  channel has subscribers, the relays already serving announce the new one with a signed
  `x.grp.relay.new`, and a subscriber joins an announced relay when the owner's signature
  verifies.
- **Edits and deletes** are sent as the channel too. Relays keep the current form: a
  subscriber who joins later gets an edited post as it now is, not marked edited, and does not
  get a deleted one.
- **Signed posts.** `send_signed_data` signs with the owner's member key over the channel's
  id and the owner's member id, as 7.1.4 does with `sign=on`. Relays forward a signed post
  naming the owner's member (unsigned ones name nobody); it still shows as the channel's. A
  signed body over 15 602 bytes is refused before it is sent, where 7.1.4 reports it sent and
  never sends it.
- **Not ported:** the relay role, owners besides the creator (so the roster the owner sends is
  always empty), member support chats, a subscriber's relay sync against the channel's link
  data on open (upstream `syncSubscriberRelays`), removing relays from a channel, and files in
  channel posts.

## Security notes

- **Channels are not end-to-end encrypted.** Every owner-to-relay and relay-to-subscriber
  connection is made without the post-quantum KEM (`PQSupportOff`), exactly as simplex-chat
  7.1.4 makes them, so the crate interoperates; turning PQ on would not. Relays read and keep
  everything posted, and replay it to new subscribers. Subscribers see the notice the official
  apps show (`e2eeInfo {"public": true, "pqEnabled": false}`). Each relay connection's
  negotiated PQ state is `ChannelRelay::pq` on the group's `channel`, so a caller can see it and
  refuse the channel. No leg can reach PQ against 7.1.4: it sends `PQEncOff` on every channel
  connection, and a ratchet gains the KEM only when both sides send with `PQEncOn`. 7.1.4's
  `joinPreparedConn'` passes `PQSupportOn` for a relay join, yet the connection it joins was
  prepared with `PQSupportOff`, whose stored keys and `pqSupport` the agent's `joinConnSrv'`
  keeps; the full trace, with module and function names, is in `ChannelRelay::pq`'s docs.

- **The store key is required.** `ClientBuilder::start` fails without `.key(…)`, and JS
  `Client.start` refuses a missing or empty `key`. The passphrase goes through Argon2id
  (19 MiB, 2 passes, 1 lane) with a random salt per store, kept in clear in the store; opening
  takes about 13 ms natively and 25 ms in wasm. A long random passphrase is still best.
- **One client per store.** `ClientBuilder::start` takes an exclusive OS lock on
  `chat.db.lock` in the client's folder and holds it until `close()` returns, so a second
  client on the same folder, in this process or another, fails with `Error::Locked` instead
  of writing the same store. `Store::open` below `Client` takes no lock.
- **Older stores** (sealed before Argon2id) still open with their old unsalted HKDF key, and
  their key is not upgraded in place. They get the counter checks and the snapshot MAC on
  their first open, but someone holding a copy from before that open can still roll rows back.
  To move one to Argon2id, start a new store.
- **Tampering at rest.** A snapshot carries a MAC over all of its bytes, so a changed,
  dropped or rolled-back row or a rewound id counter is refused when it opens. Swapping in a
  whole older snapshot is not caught. Each `saveChanges()` carries a MAC too, chained to the
  snapshot and the change before it, so a changed, missing, repeated or reordered one is
  refused; leaving off the newest ones is not caught, as an older snapshot is not. Changes of
  an older snapshot are skipped, not refused. The SQLite store checks its id counters against a sealed
  copy and overwrites deleted rows (`secure_delete`).
- **Limitation:** in SQLite, putting back an older sealed row (one row, or the counters' sealed copy with its counters) is not detected, because nothing MACs the whole file.

## Architecture

```
 Client            high-level API: builder, events, requests
   |
 engine            threads, subscriptions, retries, the event stream
   |
 chat              chat messages, contacts, groups, files, calls
   |
 agent (ratchet)   connections, double ratchet with PQ KEM
   |
 smp               SMP commands and queues
   |
 tls (h2)          TCP + TLS transport

 xftp              file chunks, crypto and descriptions (over h2), used by chat for files
 store             SQLite store, sealed with a passphrase
 worker/           Cloudflare Worker: WebSocket to TCP pipe for the browser build (TypeScript)
 crypto, encoding, types   shared by every layer
```

## Feature flags

| Feature | Default | What it enables |
|---|---|---|
| `native` | yes | Threads, TCP and TLS, the SQLite store, the engine and `Client` |
| `wasm` | no | The client in a browser, Node.js or Bun, through a WebSocket tunnel or TCP; use with `--no-default-features` |
| `testing` | no | The in-memory SMP server and agent test doubles (`agent::testing`) and `ClientBuilder::start_in_memory`, for end-to-end tests without a network |

```rust
// [dev-dependencies] simplex-client = { git = "…", features = ["testing"] }
let server = simplex_client::agent::testing::FakeSmp::new();
let alice = Client::builder(dir_a).key("k").start_in_memory(&server)?;
let bob = Client::builder(dir_b).key("k").start_in_memory(&server)?;
bob.connect(&alice.create_invite()?)?;
```

## Protocol versions tracked

Ported from simplex-chat v7.1.4 and simplexmq commit `fdbcb24d`. Newer upstream changes are
not followed automatically.

## Development

```sh
nix develop                    # pinned toolchain (1.99.1, wasm32 target)
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo clippy --features testing --all-targets -- -D warnings
cargo test --features testing --test in_memory
cargo clippy --target wasm32-unknown-unknown --no-default-features --features wasm --all-targets -- -D warnings
cargo test --target wasm32-unknown-unknown --no-default-features --features wasm
cargo deny check
```

Wasm tests need `wasm-bindgen-cli` 0.2.130 (`wasm-bindgen-test-runner`) and node, and
`scripts/build-js.sh` uses binaryen's `wasm-opt`; `nix develop` has all three. The MSRV is
Rust 1.95. See [AGENTS.md](AGENTS.md) for the code style.

Live interop tests are `#[ignore]`d. They need a local `smp-server` and `xftp-server`:

```sh
export SIMPLEX_TEST_SMP='smp://<fingerprint>@127.0.0.1:<port>'
export SIMPLEX_TEST_XFTP='xftp://<fingerprint>@127.0.0.1:<port>'
cargo test -- --ignored
```

The channel tests (`tests/it/channel_live.rs`) also need official `simplex-chat` 7.1.4 CLIs as
two relays, an owner and a subscriber, all on local servers; the setup, the last recorded
run and the PQ state measured on each leg are in
[docs/channels-live.md](docs/channels-live.md).

## License

AGPL-3.0-only, see [LICENSE](LICENSE). The code is ported from SimpleX Chat and simplexmq,
which are AGPL-3.0, so this port carries the same license. If you run a modified version as a
network service, the AGPL requires you to offer its source to your users.

## Credits

The protocol and the original implementation are the work of
[SimpleX Chat](https://github.com/simplex-chat). This crate would not exist without
[simplex-chat](https://github.com/simplex-chat/simplex-chat) and
[simplexmq](https://github.com/simplex-chat/simplexmq).
