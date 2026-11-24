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
- A `wasm32-unknown-unknown` build of the whole client, reaching relays through a WebSocket tunnel
- Low-level modules stay public if you need to go below `Client`

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
    let client = Client::builder("./bot-data").name("Echo Bot").key("store passphrase").start()?;
    println!("my address: {}", client.create_address()?);
    for event in client.events() {
        match event {
            Event::Messages(msgs) => for m in msgs { if !m.mine() { client.reply(&m, &m.text())?; } }
            Event::Chats { requests, .. } => for r in requests { client.accept(r.id)?; }
            _ => {}
        }
    }
    Ok(())
}
```

Requests on `Client`: `send`, `reply`, `edit`, `delete`, `react`, `accept`, `reject`,
`connect`, `create_invite`, `create_address`, `new_group`, `add_member`, `join_group`,
`leave_group`, `send_file`, `receive_file`, `history`, `mark_read`. `request(Request)` is the
escape hatch for anything else.

### Events

| Event | Fires when |
|---|---|
| `Event::Messages` | new messages arrive, or your own are echoed back |
| `Event::Chats` | contacts, groups or incoming contact requests change |
| `Event::Files` | a file transfer starts, progresses, finishes or fails |
| `Event::Calls` | a call is offered, answered or ended |

The exact variants are listed in the crate docs (`cargo doc --open`).

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
import init, { Client } from "./pkg/simplex_client.js";
await init();
const client = await Client.start({ name: "Alice", key: "passphrase", tunnel: "wss://<worker>/v1" });
client.on("message", (msg) => { if (!msg.mine) client.reply(msg, msg.text); });
client.on("request", (req) => client.accept(req.id));
console.log(await client.createAddress());
const snapshot = client.save(); // sealed store bytes; pass back as `snapshot` to start again
```

[`examples/browser`](examples/browser) is a minimal chat page with the full JS API.

### Node.js

The same build runs on Node.js 22+. There no tunnel is needed: the client dials relays over
direct TCP (`node:net`), and TLS still runs inside wasm. Install from git (building needs
Rust with the `wasm32-unknown-unknown` target and `wasm-bindgen-cli` 0.2.130):

```sh
npm install github:abdulrahman1s/simplex-client
```

```js
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { Client } from "simplex-client";

const saved = existsSync("bot.snapshot") ? new Uint8Array(readFileSync("bot.snapshot")) : undefined;
const bot = await Client.start({ name: "Echo Bot", key: "passphrase", snapshot: saved });
bot.on("request", (req) => bot.accept(req.id));
bot.on("message", (msg) => { if (!msg.mine && msg.text) bot.reply(msg, msg.text); });
bot.on("change", () => writeFileSync("bot.snapshot", bot.save()));
console.log("my address:", await bot.createAddress());
```

`transport: "tcp"` picks direct TCP explicitly; a `tunnel` URL uses the WebSocket tunnel
instead. `scripts/build-js.sh` builds both packages (`pkg/web` and `pkg/node`), and
[`examples/node/bot.mjs`](examples/node/bot.mjs) is the bot above with a safer save.

## Browser tunnel (Cloudflare Worker)

Browsers cannot open raw TCP. [`worker/`](worker/README.md) is a small Cloudflare Worker that
only pipes WebSocket frames to a relay's TCP port and back. It is the transport the browser
build uses (`tunnel: "wss://…/v1"`).

- **The Worker sees only TLS ciphertext.** The client runs TLS 1.3 in wasm and pins the relay
  by the fingerprint in its `smp://` address. SMP queue encryption and the end-to-end double
  ratchet sit on top of that.
- **Cloudflare can see** your IP, the relay host and port, and the timing and sizes of the
  fixed 16 KiB SMP blocks.
- **Cloudflare cannot see** message content, keys, contacts or which queues you use.
- **It is not an open proxy.** It allows only ports 5223 and 443, public hosts, and a TLS
  ClientHello with ALPN `smp/1` or `xftp/1` and no SNI. Every IP is rate-limited and has a cap
  on open tunnels.

Deploy your own:

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
| `wasm` | no | The client in a browser or node, through a WebSocket tunnel; use with `--no-default-features` |

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
