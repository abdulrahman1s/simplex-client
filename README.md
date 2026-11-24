# simplex-client

A SimpleX Chat protocol client in Rust: connect, chat, run groups and send files through a
small, plain API.

[![CI](https://github.com/abdulrahman1s/simplex-client/actions/workflows/ci.yml/badge.svg)](https://github.com/abdulrahman1s/simplex-client/actions/workflows/ci.yml)
[![License: AGPL-3.0-only](https://img.shields.io/badge/license-AGPL--3.0--only-blue.svg)](LICENSE)

> **Unofficial and unaudited.** This project is not affiliated with or endorsed by SimpleX
> Chat Ltd. It has not had a security review. Do not rely on it where a flaw would hurt someone.

## Features

- SMP queues over TLS, with the agent's double ratchet and the post-quantum KEM
- Chat messages, contacts, groups, replies, edits, deletes and reactions
- XFTP file upload and download, with file crypto and descriptions
- Calls (signalling and frame crypto)
- SQLite store sealed with a passphrase
- A threaded engine and a high-level `Client`; no async runtime
- A `wasm32-unknown-unknown` build of the whole client for browsers, Node.js and Bun, reaching
  relays through a WebSocket tunnel or direct TCP
- Low-level modules stay public if you need to go below `Client`

## Why this library

The official way to embed SimpleX Chat is its Haskell core: link `libsimplex` with its GHC
runtime, or run the `simplex-chat` CLI as a separate process and drive it over its WebSocket
API, which is what the official TypeScript client does. This crate speaks the same protocols
natively instead.

| | Official core | simplex-client |
|---|---|---|
| Native, x86_64 Linux | `libsimplex` + GHC runtime: **238 MB** · `simplex-chat` CLI: **87 MB** | a whole echo bot, SQLite included: **8.4 MB** (stripped release) |
| Browser | no | **4.4 MB** wasm, **1.3 MB** gzipped |
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
- **Private message routing.** Messages go to the relay directly.
- **Files in the browser.** The wasm build has no XFTP file transfers yet.
- **Coverage of newer features.** Features added after 7.1.4 are not tracked.

Sizes were measured on 2026-11-24: the official 7.1.4 CLI release asset, and `libsimplex`
7.0.3 with its shared libraries as packaged on Linux.

## Quick start

The crate is not published on crates.io. Depend on it through git:

```toml
[dependencies]
simplex-client = { git = "https://github.com/abdulrahman1s/simplex-client" }
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
            Event::FriendAdd(friend) => friend.send("Hi! I echo what you write.")?,
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
| `Raw(event)` | anything else the engine reports, and every chat list |

The engine's own events are `simplex_client::types::Event` (`cargo doc --open`).

## WebAssembly

The `wasm` feature builds the whole client for `wasm32-unknown-unknown`: the engine, the
agent and an in-memory store, driven by one `setTimeout` loop, plus the codec bindings
(`src/wasm/`). A browser has no raw TCP, so relays are reached through a WebSocket tunnel, a
Cloudflare Worker that only pipes bytes. TLS runs inside wasm, pinned by the relay's
fingerprint, so the tunnel sees ciphertext only. Files (XFTP) stay native-only.

```sh
cargo build --lib --release --target wasm32-unknown-unknown --no-default-features --features wasm
wasm-bindgen --target web --out-dir pkg target/wasm32-unknown-unknown/release/simplex_client.wasm
```

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

`client.save()` returns the sealed store; pass it back as `snapshot` to pick up where you left
off. JS events are named in camelCase: `ready`, `message`, `messageUpdate`, `request`,
`friendAdd`, `friendRemove`, `groupAdd`, `groupRemove`, `groupInvite`, `memberAdd`, `offline`,
`online`, `change` (save now) and `raw` (every engine event). `client.friends` and
`client.groups` are `Map`s of `Friend` and `Group`, and `await client.createGroup(name)` gives
a `Group`.

[`examples/browser`](examples/browser) is a minimal chat page with the full JS API.

### Node.js and Bun

The same build runs on Node.js 22+ and Bun. There no tunnel is needed: the client dials
relays over direct TCP (`node:net`), and TLS still runs inside wasm. Install from git
(building needs Rust with the `wasm32-unknown-unknown` target and `wasm-bindgen-cli` 0.2.130):

```sh
npm install github:abdulrahman1s/simplex-client
bun add github:abdulrahman1s/simplex-client
```

```js
import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { Client } from "simplex-client";

const SNAPSHOT = "bot.snapshot";

const bot = await Client.start({
  name: "Echo Bot",
  key: "passphrase",
  snapshot: existsSync(SNAPSHOT) ? readFileSync(SNAPSHOT) : undefined,
});

bot.on("request", (request) => request.accept());
bot.on("friendAdd", (friend) => friend.send("Hi! I echo what you write."));
bot.on("message", async (message) => {
  if (message.mine || !message.text) return;
  await message.reply(message.text);
});

bot.on("change", () => writeFileSync(SNAPSHOT, bot.save()));
console.log("My address:", await bot.createAddress());
```

`transport: "tcp"` picks direct TCP explicitly; a `tunnel` URL uses the WebSocket tunnel
instead. `scripts/build-js.sh` builds both packages (`pkg/web` and `pkg/node`), and
[`examples/node/bot.mjs`](examples/node/bot.mjs) is the bot above with a safer save; run it
with `node examples/node/bot.mjs` or `bun examples/node/bot.mjs`.

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
- **It is not an open proxy.** It allows only ports 5223 and 443, public hosts, and a TLS
  ClientHello with ALPN `smp/1` or `xftp/1` and no SNI. Every IP is rate-limited and has a cap
  on open tunnels.

Deploy your own and pass its URL as `tunnel`, with one click:

[![Deploy to Cloudflare](https://deploy.workers.cloudflare.com/button)](https://deploy.workers.cloudflare.com/?url=https://github.com/abdulrahman1s/simplex-client/tree/main/worker)

or from the command line:

```sh
cd worker && npm ci
npx wrangler login
npx wrangler deploy
```

The protocol, limits and settings are in [worker/README.md](worker/README.md).

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

## Protocol versions tracked

Ported from simplex-chat v7.1.4 and simplexmq commit `fdbcb24d`. Newer upstream changes are
not followed automatically.

## Development

```sh
nix develop                    # pinned toolchain (1.99.1, wasm32 target)
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test
cargo clippy --target wasm32-unknown-unknown --no-default-features --features wasm --all-targets -- -D warnings
cargo test --target wasm32-unknown-unknown --no-default-features --features wasm
cargo deny check
```

Wasm tests need `wasm-bindgen-cli` 0.2.130 (`wasm-bindgen-test-runner`) and node. The MSRV is
Rust 1.95. See [AGENTS.md](AGENTS.md) for the code style.

Live interop tests are `#[ignore]`d. They need a local `smp-server` and `xftp-server`:

```sh
export SIMPLEX_TEST_SMP='smp://<fingerprint>@127.0.0.1:<port>'
export SIMPLEX_TEST_XFTP='xftp://<fingerprint>@127.0.0.1:<port>'
cargo test -- --ignored
```

## License

AGPL-3.0-only, see [LICENSE](LICENSE). The code is ported from SimpleX Chat and simplexmq,
which are AGPL-3.0, so this port carries the same license. If you run a modified version as a
network service, the AGPL requires you to offer its source to your users.

## Credits

The protocol and the original implementation are the work of
[SimpleX Chat](https://github.com/simplex-chat). This crate would not exist without
[simplex-chat](https://github.com/simplex-chat/simplex-chat) and
[simplexmq](https://github.com/simplex-chat/simplexmq).
